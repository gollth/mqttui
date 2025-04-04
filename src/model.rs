use std::collections::BTreeMap;

use serde_json::Value;

#[derive(Default, Debug)]
pub struct Model {
    pub shutdown: bool,
    pub counter: i32,

    messages: BTreeMap<Topic, Message>,
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

pub fn update(state: &mut Model, event: Event) -> Option<Event> {
    match event {
        Event::Quit => state.shutdown = true,
        Event::Up => state.counter += 1,
        Event::Down => state.counter -= 1,
        Event::Message(Message { topic, data }) => {
            state.counter += 1;
            state
                .messages
                .entry(topic.clone())
                .and_modify(|msg| msg.data = data.clone())
                .or_insert(Message { topic, data });
        }
    }
    None
}
