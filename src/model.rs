use std::collections::BTreeMap;

use ratatui::widgets::ListState;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct Model {
    pub shutdown: bool,
    pub counter: i32,

    messages: BTreeMap<Topic, Message>,
    pub state_topics: ListState,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Quit,
    Up,
    Down,
    Message(Message),
}

pub type Topic = String;

#[derive(Debug, PartialEq, Eq)]
pub struct Message {
    pub(crate) topic: Topic,
    pub(crate) data: Value,
}

impl Model {
    pub fn update(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::Quit => self.shutdown = true,
            Event::Up => self.state_topics.select_previous(),
            Event::Down => self.state_topics.select_next(),
            Event::Message(Message { topic, data }) => {
                self.counter += 1;
                self.messages
                    .entry(topic.clone())
                    .and_modify(|msg| msg.data = data.clone())
                    .or_insert(Message { topic, data });
                if self.messages.is_empty() {
                    self.state_topics.select(None);
                } else if self.state_topics.selected().is_none() {
                    self.state_topics.select(Some(0));
                }
            }
        }
        None
    }

    pub fn topics(&self) -> impl Iterator<Item = &Topic> {
        self.messages.keys()
    }
}
