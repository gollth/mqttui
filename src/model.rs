use std::{
    borrow::Cow,
    cmp::Reverse,
    collections::{BTreeMap, HashSet, VecDeque},
    ops::Deref,
    rc::Rc,
    time::Instant,
};

use clipboard::{ClipboardContext, ClipboardProvider};
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use derivative::Derivative;
use enum_as_inner::EnumAsInner;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
};
use serde_json::Value;
use tracing::{Level, instrument};
use url::Url;

use crate::{
    config::Config,
    events::{Event, RenderEvent, UpdateEvent},
    highlight::Highlighter,
    jq::{History, Jaqqer},
    ui::SCROLL_BOTTOM_OFFSET,
};

pub struct Model {
    config: Config,
    highlighter: Highlighter,

    pub connected: bool,
    pub shutdown: bool,
    broker: Url,

    mode: Mode,
    messages: BTreeMap<String, Messages>,
    selection: Option<String>,

    clipboard: ClipboardContext,
    copy: usize,
    history: History,
}

pub struct Messages {
    topic: Topic,
    latest: Message,
    queue: VecDeque<Message>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Topic {
    name: String,
    highlights: HashSet<usize>,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub(crate) topic: Topic,
    pub(crate) data: Result<Value, String>,
    pub(crate) text: String,
    pub(crate) retain: bool,
    last: Instant,
}

#[derive(Debug, Clone, EnumAsInner)]
pub enum Mode {
    Topics {
        filter: Option<Filter>,
    },
    Detail {
        topic: String,
        index: Option<usize>,
        scroll: u16,
        jq: Jaqqer,
    },
}

#[derive(Clone, Derivative, EnumAsInner)]
#[derivative(Debug, PartialEq)]
pub enum Filter {
    Keep {
        pattern: String,
        cursor: u16,
        #[derivative(Debug = "ignore", PartialEq = "ignore")]
        fuzzer: Rc<SkimMatcherV2>,
    },
    Skip {
        pattern: String,
        cursor: u16,
    },
}

impl Model {
    pub fn new(broker: Url) -> Result<Self> {
        let config = Config::load()?;
        Ok(Self {
            highlighter: Highlighter::new(&config)?,
            config,
            broker,
            clipboard: ClipboardProvider::new().map_err(|e| eyre!("{e}"))?,
            connected: false,
            shutdown: false,
            copy: 0,
            history: History::load()?,
            messages: Default::default(),
            selection: None,
            mode: Mode::Topics {
                filter: Default::default(),
            },
        })
    }

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }

    pub fn topics(&self) -> impl Iterator<Item = (&String, &Message)> {
        self.messages
            .iter()
            .enumerate()
            .filter_map(|(i, (topic, message))| {
                let i = -(i as i64);
                let Some(filter) = self.mode().as_topics().and_then(|f| f.as_ref()) else {
                    return Some((i, topic, message));
                };

                let score = filter.score(i, topic)?;
                Some((score, topic, message))
            })
            .sorted_by_key(|(score, _, _)| Reverse(*score))
            .map(|(_, topic, m)| (topic, &m.latest))
    }

    pub fn message(&self, topic: &str, index: Option<usize>) -> Option<Cow<str>> {
        let m = self.messages.get(topic)?.get(index)?;
        Some(
            self._message(m)
                .map(Cow::Owned)
                .unwrap_or(Cow::Borrowed(&m.text)),
        )
    }

    pub fn message_count(&self, topic: &str) -> usize {
        self.messages
            .get(topic)
            .map(|m| m.len())
            .unwrap_or_default()
    }

    fn _message(&self, message: &Message) -> Option<String> {
        let data = message.data.as_ref().ok()?;
        let (.., jq) = self.mode.as_detail()?;
        let x = jq
            .run(data.clone())
            .ok()?
            .into_iter()
            .filter_map(|x| serde_json::to_string_pretty(&x).ok())
            .join("\n");
        Some(x)
    }

    pub fn error(&self, topic: &str, index: Option<usize>) -> Option<&str> {
        self.messages
            .get(topic)?
            .get(index)?
            .data
            .as_ref()
            .err()
            .map(|e| e.as_str())
    }

    pub fn broker(&self) -> &Url {
        &self.broker
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn is_copy(&self) -> bool {
        self.copy > 0
    }

    pub(crate) fn highlight<'a>(&self, text: &'a str, area: Rect, offset: u16) -> Text<'a> {
        text.lines()
            .enumerate()
            .map(|(i, line)| {
                if offset <= i as u16 && i as u16 <= offset + area.height {
                    // Is visible on scroll track, so color it
                    self.highlighter.highlight(line)
                } else {
                    Line::default()
                }
            })
            .collect()
    }

    pub fn update(&mut self, event: Event) {
        let keys = self.config().keys.clone();
        self.mode = match self.mode.clone() {
            Mode::Topics { mut filter } => {
                let insert = filter.is_some();
                match event {
                    Event::Update(UpdateEvent::Receive(message)) => {
                        self.on_message(message.into());
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Tick) => {
                        self.copy = self.copy.saturating_sub(1);
                        Mode::Topics { filter }
                    }

                    Event::Render(RenderEvent::Connect) => {
                        self.connected = true;
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Disconnect) => {
                        self.connected = false;
                        Mode::Topics { filter }
                    }

                    // Quit applicaton
                    Event::Render(RenderEvent::Quit) => {
                        self.shutdown = true;
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Char('q')) if !insert => {
                        self.shutdown = true;
                        Mode::Topics { filter }
                    }

                    // Enter into & out of message pane
                    Event::Render(RenderEvent::Select) => match self.selection() {
                        None => Mode::Topics { filter },
                        Some(topic) => Mode::Detail {
                            topic: topic.into(),
                            jq: Jaqqer::default(),
                            scroll: 0,
                            index: None,
                        },
                    },
                    Event::Render(RenderEvent::Back) => {
                        self.clear_filter();
                        Mode::Topics { filter: None }
                    }

                    // Navigation
                    Event::Render(RenderEvent::Up) => {
                        self.select_next();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Char('k')) if !insert => {
                        self.select_next();
                        Mode::Topics { filter }
                    }

                    Event::Render(RenderEvent::Down) => {
                        self.select_previous();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Char('j')) if !insert => {
                        self.select_previous();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Home) if !insert => {
                        self.select_first();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::End) if !insert => {
                        self.select_last();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::ScrollDown | RenderEvent::ScrollUp) => {
                        /* not yet supported */
                        Mode::Topics { filter }
                    }

                    Event::Render(RenderEvent::Backspace) if insert => {
                        if let Some(filter) = filter.as_mut() {
                            filter.backspace();
                        }
                        self.apply_filter();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Delete) if insert => {
                        if let Some(filter) = filter.as_mut() {
                            filter.delete();
                        }
                        self.apply_filter();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Backspace) => Mode::Topics { filter },
                    Event::Render(RenderEvent::Delete) => Mode::Topics { filter },

                    // Copy topic
                    Event::Render(RenderEvent::Char(c)) if !insert && keys.copy == c => {
                        if let Some(msg) = self.selection() {
                            let _ = self.clipboard.set_contents(msg.into());
                            self.copy += 2;
                        }
                        Mode::Topics { filter }
                    }

                    // Searching
                    Event::Render(RenderEvent::Char(c)) if !insert && keys.search == c => {
                        Mode::Topics {
                            filter: Some(Filter::keep()),
                        }
                    }
                    Event::Render(RenderEvent::Char(c)) if !insert && keys.ignore == c => {
                        Mode::Topics {
                            filter: Some(Filter::skip()),
                        }
                    }

                    // Text input
                    Event::Render(RenderEvent::Char(c)) if insert => {
                        if let Some(filter) = filter.as_mut() {
                            filter.insert(c)
                        }
                        self.apply_filter();
                        Mode::Topics { filter }
                    }
                    Event::Render(RenderEvent::Left) => Mode::Topics {
                        filter: filter.map(|f| f.move_cursor(-1)),
                    },
                    Event::Render(RenderEvent::Right) => Mode::Topics {
                        filter: filter.map(|f| f.move_cursor(1)),
                    },
                    Event::Render(RenderEvent::Home) if insert => Mode::Topics {
                        filter: filter.map(|f| f.move_cursor(-100)),
                    },
                    Event::Render(RenderEvent::End) if insert => Mode::Topics {
                        filter: filter.map(|f| f.move_cursor(100)),
                    },
                    Event::Render(RenderEvent::Char(_)) => Mode::Topics { filter },
                    Event::Render(RenderEvent::Home | RenderEvent::End) => Mode::Topics { filter },
                }
            }
            Mode::Detail {
                topic,
                scroll,
                index,
                jq,
            } => match event {
                // Update
                Event::Update(UpdateEvent::Receive(message)) => {
                    self.on_message(message.into());
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }
                Event::Render(RenderEvent::Tick) => {
                    self.copy = self.copy.saturating_sub(1);
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }
                Event::Render(RenderEvent::Connect) => {
                    self.connected = true;
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }
                Event::Render(RenderEvent::Disconnect) => {
                    self.connected = false;
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }

                // Filtering
                Event::Render(RenderEvent::Char(c))
                    if !jq.is_prompt()
                        && keys.search == c
                        && self.error(&topic, index).is_none() =>
                {
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq: jq.edit(&mut self.history),
                    }
                }
                Event::Render(RenderEvent::Back) if !jq.is_dormant() => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.clear(),
                },
                Event::Render(RenderEvent::Select) if jq.is_prompt() && jq.errors().is_empty() => {
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq: jq.activate(&mut self.history),
                    }
                }

                // Quit
                Event::Render(RenderEvent::Quit) => {
                    self.shutdown = true;
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }
                Event::Render(RenderEvent::Char('q')) if !jq.is_prompt() => {
                    self.shutdown = true;
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }

                // Clear any history offset
                Event::Render(RenderEvent::Back) if index.is_some() => Mode::Detail {
                    topic,
                    scroll,
                    index: None,
                    jq,
                },

                // Back to topics overview
                Event::Render(RenderEvent::Back) => {
                    self.clear_filter();
                    Mode::Topics { filter: None }
                }

                // Copy
                Event::Render(RenderEvent::Char(c)) if !jq.is_prompt() && keys.copy == c => {
                    if let Some(msg) = self.message(&topic, index) {
                        let _ = self.clipboard.set_contents(msg.into());
                        self.copy += 2;
                    }
                    Mode::Detail {
                        topic,
                        scroll,
                        index,
                        jq,
                    }
                }

                // Navigation
                // up
                Event::Render(RenderEvent::Up) if !jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_sub(1),
                    index,
                    jq,
                },
                Event::Render(RenderEvent::Up) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.up(&self.history),
                },
                Event::Render(RenderEvent::Char('k')) if !jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_sub(1),
                    index,
                    jq,
                },
                Event::Render(RenderEvent::ScrollUp) => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_sub(self.config().topics.lines_to_scroll),
                    index,
                    jq,
                },
                Event::Render(RenderEvent::Char('{')) if !jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_sub(self.config().topics.lines_to_scroll),
                    index,
                    jq,
                },

                // down
                Event::Render(RenderEvent::Down) if !jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_add(1),
                    index,
                    jq,
                },
                Event::Render(RenderEvent::Down) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.down(&self.history),
                },
                Event::Render(RenderEvent::Char('j')) if !jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_add(1),
                    index,
                    jq,
                },
                Event::Render(RenderEvent::ScrollDown) => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_add(self.config().topics.lines_to_scroll),
                    index,
                    jq,
                },
                Event::Render(RenderEvent::Char('}')) if !jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll: scroll.saturating_add(self.config().topics.lines_to_scroll),
                    index,
                    jq,
                },

                Event::Render(RenderEvent::Char('l')) if !jq.is_prompt() => Mode::Detail {
                    scroll,
                    index: Some(
                        index
                            .unwrap_or_else(|| self.message_count(&topic))
                            .saturating_sub(1),
                    ),
                    topic,
                    jq,
                },
                Event::Render(RenderEvent::Char('h')) if !jq.is_prompt() => Mode::Detail {
                    index: index
                        .map(|i| i + 1)
                        .filter(|i| *i < self.message_count(&topic)),
                    topic,
                    scroll,
                    jq,
                },
                Event::Render(RenderEvent::Right) if !jq.is_prompt() => Mode::Detail {
                    index: index
                        .map(|i| i + 1)
                        .filter(|i| *i < self.message_count(&topic)),
                    topic,
                    scroll,
                    jq,
                },
                Event::Render(RenderEvent::Left) if !jq.is_prompt() => Mode::Detail {
                    scroll,
                    index: Some(
                        index
                            .unwrap_or_else(|| self.message_count(&topic))
                            .saturating_sub(1),
                    ),
                    topic,
                    jq,
                },
                Event::Render(RenderEvent::Left) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.move_cursor(-1),
                },
                Event::Render(RenderEvent::Right) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.move_cursor(1),
                },
                Event::Render(RenderEvent::Home) if jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.move_cursor(-100),
                },
                Event::Render(RenderEvent::End) if jq.is_prompt() => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.move_cursor(100),
                },
                Event::Render(RenderEvent::Home) => Mode::Detail {
                    topic,
                    scroll: 0,
                    index,
                    jq,
                },
                Event::Render(RenderEvent::End) => Mode::Detail {
                    scroll: self
                        .message(&topic, index)
                        .unwrap_or_default()
                        .lines()
                        .count()
                        .saturating_sub(SCROLL_BOTTOM_OFFSET) as u16,
                    topic,
                    index,
                    jq,
                },

                // Text input
                Event::Render(RenderEvent::Char(c)) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.input(c, &mut self.history),
                },
                Event::Render(RenderEvent::Backspace) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.backspace(&mut self.history),
                },
                Event::Render(RenderEvent::Delete) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq: jq.delete(&mut self.history),
                },

                // Enter on no prompt, just stay
                Event::Render(RenderEvent::Select) => Mode::Detail {
                    topic,
                    scroll,
                    index,
                    jq,
                },
            },
        };
    }

    #[instrument(skip(self))]
    fn apply_filter(&mut self) {
        match self.mode() {
            Mode::Topics {
                filter: Some(filter),
            } => {
                let highlights = self
                    .messages
                    .keys()
                    .map(|topic| filter.highlights(topic))
                    .collect::<Vec<_>>();
                for (m, highlights) in self.messages.values_mut().zip(highlights) {
                    m.topic.highlights = highlights;
                }

                if !self
                    .topics()
                    .any(|(topic, _)| self.selection().is_some_and(|s| s == topic))
                    && self.topics().count() > 0
                {
                    let first = self.topics().next().map(|(topic, _)| topic.clone());
                    self.selection = first;
                }
            }
            Mode::Detail { jq, .. } if jq.is_active() => {}
            _ => {}
        }
    }

    #[instrument(skip(self))]
    fn clear_filter(&mut self) {
        for m in self.messages.values_mut() {
            m.topic.highlights.clear();
        }
        let Mode::Topics { filter, .. } = &mut self.mode else {
            return;
        };
        filter.take();
    }

    #[instrument(skip(self))]
    fn select_first(&mut self) {
        let next = self.topics().next().map(|(t, _)| t.clone());
        self.selection = next;
    }

    #[instrument(skip(self))]
    fn select_last(&mut self) {
        let last = self.topics().last().map(|(t, _)| t.clone());
        self.selection = last;
    }

    #[instrument(skip(self))]
    fn select_next(&mut self) {
        let next = self
            .topics()
            .position(|(t, _)| self.selection.as_deref().is_some_and(|s| s == t.as_str()))
            .map(|n| (n.saturating_sub(1)).max(0))
            .unwrap_or(0);
        let next = self.topics().nth(next).map(|(topic, _)| topic.clone());
        self.selection = next;
    }

    #[instrument(skip(self))]
    fn select_previous(&mut self) {
        let previous = self
            .topics()
            .position(|(t, _)| self.selection.as_deref().is_some_and(|s| s == t.as_str()))
            .map(|n| (n + 1).min(self.topics().count().saturating_sub(1)))
            .unwrap_or(0);
        let previous = self.topics().nth(previous).map(|(topic, _)| topic.clone());
        self.selection = previous;
    }

    #[instrument(skip_all, level = Level::DEBUG, fields(topic = message.topic.as_str(),  retain = message.retain))]
    fn on_message(&mut self, message: Message) {
        self.messages
            .entry(message.topic.as_str().into())
            .and_modify(|m| m.update(message.clone(), &self.config))
            .or_insert(message.into());

        if self.messages.is_empty() {
            self.selection = None;
        } else if self.selection().is_none() {
            self.select_next();
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

impl Messages {
    fn len(&self) -> usize {
        self.queue.len() + 1
    }

    fn update(&mut self, mut message: Message, config: &Config) {
        let was_retain = self.latest.retain;
        if was_retain {
            message.retain = was_retain;
        }
        self.queue.push_back(message);
        std::mem::swap(self.queue.back_mut().unwrap(), &mut self.latest);

        if config.topics.buffer_size > 0 && self.len() >= config.topics.buffer_size {
            self.queue.pop_front();
        }
    }

    fn get(&self, index: Option<usize>) -> Option<&Message> {
        if index.is_none() || index == Some(self.queue.len()) {
            return Some(&self.latest);
        }
        self.queue.get(index?)
    }
}

impl From<Message> for Messages {
    fn from(value: Message) -> Self {
        Self {
            topic: value.topic.clone(),
            latest: value,
            queue: Default::default(),
        }
    }
}

impl Message {
    pub(crate) fn freshness(&self, config: &Config) -> Color {
        if self.retain {
            return config.colors.retain;
        }
        let ttl = Instant::now() - self.last;
        if ttl < config.topics.fresh_until {
            return config.colors.fresh;
        }
        if ttl < config.topics.stale_after {
            return config.colors.intime;
        }
        config.colors.stale
    }
}

impl From<rumqttc::Publish> for Message {
    fn from(value: rumqttc::Publish) -> Self {
        let message = serde_json::from_slice::<serde_json::Value>(&value.payload)
            .wrap_err("Message is not proper JSON");
        Self {
            topic: Topic {
                name: value.topic,
                highlights: Default::default(),
            },
            text: message
                .as_ref()
                .ok()
                .and_then(|message| serde_json::to_string_pretty(&message).ok())
                .or_else(|| String::from_utf8(value.payload.into()).ok())
                .unwrap_or_default(),
            data: message.map_err(|e| format!("{e:#}")),
            retain: value.retain,
            last: Instant::now(),
        }
    }
}

impl Filter {
    fn keep() -> Self {
        Self::Keep {
            pattern: Default::default(),
            fuzzer: Default::default(),
            cursor: 0,
        }
    }

    fn skip() -> Self {
        Self::Skip {
            pattern: Default::default(),
            cursor: 0,
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
            Self::Keep { pattern, .. } => pattern.as_str(),
            Self::Skip { pattern, .. } => pattern.as_str(),
        }
    }

    pub(crate) fn cursor(&self) -> u16 {
        match self {
            Self::Keep { cursor, .. } => *cursor,
            Self::Skip { cursor, .. } => *cursor,
        }
    }

    fn insert(&mut self, c: char) {
        match self {
            Self::Keep {
                pattern, cursor, ..
            }
            | Self::Skip { pattern, cursor } => {
                pattern.insert(*cursor as usize, c);
                *cursor += 1;
            }
        };
    }

    fn backspace(&mut self) {
        match self {
            Self::Keep {
                pattern, cursor, ..
            }
            | Self::Skip {
                pattern, cursor, ..
            } => {
                if !pattern.is_empty() && *cursor > 0 {
                    pattern.remove(*cursor as usize - 1);
                    *cursor -= 1;
                }
            }
        };
    }

    fn delete(&mut self) {
        match self {
            Self::Keep {
                pattern, cursor, ..
            }
            | Self::Skip {
                pattern, cursor, ..
            } => {
                let c = *cursor as usize;
                if !pattern.is_empty() && c < pattern.chars().count() {
                    pattern.remove(c);
                }
            }
        };
    }

    fn move_cursor(mut self, offset: i16) -> Self {
        match &mut self {
            Self::Keep {
                pattern, cursor, ..
            }
            | Self::Skip { pattern, cursor } => {
                *cursor =
                    ((*cursor as i16) + offset).clamp(0, pattern.chars().count() as i16) as u16;
            }
        }
        self
    }

    fn highlights(&self, haystack: &str) -> HashSet<usize> {
        match self {
            // Use cached values
            Self::Keep {
                pattern, fuzzer, ..
            } => fuzzer
                .fuzzy_indices(haystack, pattern)
                .into_iter()
                .flat_map(|(_, xs)| xs)
                .collect(),
            Self::Skip { .. } => Default::default(),
        }
    }

    fn score(&self, i: i64, haystack: &str) -> Option<i64> {
        match self {
            Self::Keep { pattern, .. } | Self::Skip { pattern, .. } if pattern.is_empty() => {
                Some(i)
            }
            Self::Keep {
                pattern, fuzzer, ..
            } => fuzzer.fuzzy_match(haystack, pattern),
            Self::Skip { pattern, .. } if !haystack.contains(pattern) => Some(i),
            Self::Skip { .. } => None,
        }
    }
}
