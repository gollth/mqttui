use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use clipboard::{ClipboardContext, ClipboardProvider};
use color_eyre::{Result, eyre::eyre};
use enum_as_inner::EnumAsInner;
use ratatui::{style::Color, widgets::ListState};
use serde_json::Value;

/// Timeout until when messages are considered fresh (i.e. white highlight)
const FRESH: Duration = Duration::from_millis(500);

/// Timeout after which messages considered to be stale (i.e. dark grey highlight)
const STALE: Duration = Duration::from_secs(5);

pub struct Model {
    pub shutdown: bool,
    pub counter: i32,

    messages: BTreeMap<Topic, Message>,
    pub state_topics: ListState,

    clipboard: ClipboardContext,
    snackbar: usize,
}

#[derive(Debug, PartialEq, EnumAsInner)]
pub enum Event {
    Render(RenderEvent),
    Update(UpdateEvent),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RenderEvent {
    Tick,
    Up,
    Down,
    Back,
    Char(char),
}

#[derive(Debug, PartialEq)]
pub enum UpdateEvent {
    Receive(Message),
}

pub type Topic = String;

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub(crate) topic: Topic,
    pub(crate) data: Value,
    pub(crate) retain: bool,
    last: Instant,
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
            state_topics: Default::default(),
        })
    }

    pub fn update(&mut self, event: Event) {
        match event {
            Event::Render(RenderEvent::Tick) => {
                self.snackbar = self.snackbar.saturating_sub(1);
            }
            Event::Render(RenderEvent::Up) | Event::Render(RenderEvent::Char('k')) => {
                self.state_topics.select_previous()
            }
            Event::Render(RenderEvent::Down) | Event::Render(RenderEvent::Char('j')) => {
                self.state_topics.select_next()
            }
            Event::Render(RenderEvent::Back) | Event::Render(RenderEvent::Char('q')) => {
                self.shutdown = true
            }
            Event::Render(RenderEvent::Char('y')) => {
                if let Some(msg) = self
                    .state_topics
                    .selected()
                    .and_then(|i| self.topics().nth(i))
                {
                    let _ = self.clipboard.set_contents(msg.topic.clone());
                    self.snackbar += 5;
                }
            }
            Event::Render(RenderEvent::Char(_)) => {}
            Event::Update(UpdateEvent::Receive(message)) => {
                self.counter += 1;
                self.messages
                    .entry(message.topic.clone())
                    .and_modify(|msg| msg.on_receive(&message.data))
                    .or_insert(message);

                if self.messages.is_empty() {
                    self.state_topics.select(None);
                } else if self.state_topics.selected().is_none() {
                    self.state_topics.select(Some(0));
                }
            }
        }
    }

    pub fn topics(&self) -> impl Iterator<Item = &Message> {
        self.messages.values()
    }

    pub fn popup(&self) -> bool {
        self.snackbar > 0
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
            topic: value.topic().into(),
            data: serde_json::from_slice(value.payload()).unwrap(),
            retain: value.retained(),
            last: Instant::now(),
        }
    }
}
