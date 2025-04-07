use std::{
    collections::{BTreeMap, HashSet},
    ops::Deref,
    time::{Duration, Instant},
};

use clipboard::{ClipboardContext, ClipboardProvider};
use color_eyre::{Result, eyre::eyre};
use enum_as_inner::EnumAsInner;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde_json::Value;

use crate::events::{Event, RenderEvent, UpdateEvent};

/// Timeout until when messages are considered fresh (i.e. white highlight)
const FRESH: Duration = Duration::from_millis(500);

/// Timeout after which messages considered to be stale (i.e. dark grey highlight)
const STALE: Duration = Duration::from_secs(5);

pub struct Model {
    pub shutdown: bool,
    pub counter: i32,

    messages: BTreeMap<String, Message>,
    selection: Option<String>,

    filter: Option<Filter>,

    clipboard: ClipboardContext,
    snackbar: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Topic {
    name: String,
    highlights: HashSet<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub(crate) topic: Topic,
    pub(crate) data: Value,
    pub(crate) retain: bool,
    last: Instant,
}

#[derive(Clone, Debug, PartialEq, EnumAsInner)]
pub enum Filter {
    Keep { pattern: String },
    Skip { pattern: String },
}

impl Model {
    pub fn new() -> Result<Self> {
        let clipboard = ClipboardProvider::new().map_err(|e| eyre!("{e}"))?;
        Ok(Self {
            clipboard,
            shutdown: false,
            counter: 0,
            snackbar: 0,
            messages: Default::default(),
            selection: Default::default(),
            filter: Default::default(),
        })
    }

    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }

    pub fn topics(&self) -> impl Iterator<Item = (&String, &Message)> {
        self.messages.iter().filter(|(t, _)| {
            self.filter
                .as_ref()
                .is_none_or(|filter| filter.satisfies(t))
        })
    }

    pub fn filter(&self) -> Option<&Filter> {
        self.filter.as_ref()
    }

    pub fn popup(&self) -> bool {
        self.snackbar > 0
    }

    pub fn update(&mut self, event: Event) {
        let insert = self.filter.is_some();
        match event {
            Event::Render(RenderEvent::Tick) => {
                self.snackbar = self.snackbar.saturating_sub(1);
            }
            Event::Render(RenderEvent::Up) => self.select_next(),
            Event::Render(RenderEvent::Char('k')) if !insert => self.select_next(),

            Event::Render(RenderEvent::Down) => self.select_previous(),
            Event::Render(RenderEvent::Char('j')) if !insert => self.select_previous(),

            Event::Render(RenderEvent::Back) if self.filter.is_some() => self.clear_filter(),
            Event::Render(RenderEvent::Back) => {}

            Event::Render(RenderEvent::Delete) if insert => {
                if let Some(filter) = self.filter.as_mut() {
                    filter.delete();
                }
                self.update_filter();
            }
            Event::Render(RenderEvent::Delete) => {}

            Event::Render(RenderEvent::Char('q')) if !insert => self.shutdown = true,
            Event::Render(RenderEvent::Char('y')) if !insert => {
                if let Some(msg) = self.selection.as_deref() {
                    let _ = self.clipboard.set_contents(msg.into());
                    self.snackbar += 5;
                }
            }
            Event::Render(RenderEvent::Char('/')) if !insert => self.filter = Some(Filter::keep()),
            Event::Render(RenderEvent::Char('?')) if !insert => self.filter = Some(Filter::skip()),

            Event::Render(RenderEvent::Char(c)) if insert => {
                if let Some(filter) = self.filter.as_mut() {
                    filter.push(c)
                }
                self.update_filter()
            }

            Event::Render(RenderEvent::Char(_)) => {}
            Event::Update(UpdateEvent::Receive(message)) => {
                self.on_message(message);
            }
        }
    }

    fn update_filter(&mut self) {
        let filter = self
            .filter
            .as_ref()
            .expect("to only call `Model::update_filter()` with an active filter set");
        for m in self.messages.values_mut() {
            m.topic.highlights = filter.highlights(&m.topic);
        }
    }

    fn clear_filter(&mut self) {
        self.filter = None;
        for m in self.messages.values_mut() {
            m.topic.highlights.clear();
        }
    }

    fn select_next(&mut self) {
        let previous = self
            .topics()
            .position(|(t, _)| self.selection.as_deref().is_some_and(|s| s == t.as_str()))
            .map(|n| (n.saturating_sub(1)).max(0))
            .unwrap_or(0);
        let next = self.topics().nth(previous).map(|(topic, _)| topic.clone());
        self.selection = next;
    }

    fn select_previous(&mut self) {
        let next = self
            .topics()
            .position(|(t, _)| self.selection.as_deref().is_some_and(|s| s == t.as_str()))
            .map(|n| (n + 1).min(self.topics().count().saturating_sub(1)))
            .unwrap_or(0);
        let next = self.topics().nth(next).map(|(topic, _)| topic.clone());
        self.selection = next;
    }

    fn on_message(&mut self, message: Message) {
        self.counter += 1;
        self.messages
            .entry(message.topic.as_str().into())
            .and_modify(|msg| msg.on_receive(&message.data))
            .or_insert(message);

        if self.messages.is_empty() {
            self.selection = None;
        }
    }
}

impl Deref for Topic {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.name.as_str()
    }
}

impl Topic {
    pub(crate) fn as_str(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn line(&self, base: Style) -> Line {
        self.name
            .char_indices()
            .map(|(i, c)| {
                Span::styled(
                    c.to_string(),
                    base.patch(if self.highlights.contains(&i) {
                        Style::new().fg(Color::Red)
                    } else {
                        Style::default()
                    }),
                )
            })
            .collect()
    }
}

impl Message {
    fn on_receive(&mut self, value: &Value) {
        self.data = value.clone();
        self.last = Instant::now();
    }

    pub(crate) fn freshness(&self) -> Color {
        if self.retain {
            return Color::Yellow;
        }
        let ttl = Instant::now() - self.last;
        if ttl < FRESH {
            return Color::White;
        }
        if ttl < STALE {
            return Color::Gray;
        }
        Color::DarkGray
    }
}

impl From<paho_mqtt::Message> for Message {
    fn from(value: paho_mqtt::Message) -> Self {
        Self {
            topic: Topic {
                name: value.topic().into(),
                highlights: Default::default(),
            },
            data: serde_json::from_slice(value.payload()).unwrap(),
            retain: value.retained(),
            last: Instant::now(),
        }
    }
}

impl Filter {
    fn keep() -> Self {
        Self::Keep {
            pattern: Default::default(),
        }
    }

    fn skip() -> Self {
        Self::Skip {
            pattern: Default::default(),
        }
    }

    pub(crate) fn kind(&self) -> &str {
        match self {
            Self::Keep { .. } => "Filter",
            Self::Skip { .. } => "Ignore",
        }
    }

    pub(crate) fn pattern(&self) -> &str {
        match self {
            Self::Keep { pattern } => pattern.as_str(),
            Self::Skip { pattern } => pattern.as_str(),
        }
    }

    fn push(&mut self, c: char) {
        match self {
            Self::Keep { pattern } => pattern.push(c),
            Self::Skip { pattern } => pattern.push(c),
        };
    }

    fn delete(&mut self) {
        match self {
            Self::Keep { pattern } => pattern.pop(),
            Self::Skip { pattern } => pattern.pop(),
        };
    }

    pub(crate) fn highlights(&self, haystack: &str) -> HashSet<usize> {
        match self {
            Self::Keep { pattern } => haystack
                .find(pattern.as_str())
                .into_iter()
                .flat_map(|start| start..(start + pattern.chars().count()))
                .collect(),
            Self::Skip { .. } => Default::default(),
        }
    }

    fn satisfies(&self, haystack: &str) -> bool {
        match self {
            Self::Keep { pattern } => haystack.contains(pattern),
            Self::Skip { pattern } => pattern.is_empty() || !haystack.contains(pattern),
        }
    }
}
