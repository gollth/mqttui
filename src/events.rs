use crossterm::event::{Event as CrossEvent, EventStream, KeyCode, KeyEventKind};
use futures::{Stream, StreamExt, stream::select};
use paho_mqtt::{AsyncClient, QoS};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task;

use crate::model::{Event, Message};

pub async fn handler(client: &mut AsyncClient) -> UnboundedReceiver<Event> {
    let (tx, rx) = unbounded_channel();
    client.subscribe("#", QoS::default());

    let stream = client.get_stream(None);
    task::spawn(async move {
        let mut events = Box::pin(select(keys(), messages(stream)));
        while let Some(event) = events.next().await {
            tx.send(event).unwrap()
        }
    });
    rx
}

fn messages(stream: impl Stream<Item = Option<paho_mqtt::Message>>) -> impl Stream<Item = Event> {
    stream
        .filter_map(|message| async move { message })
        .map(|message| {
            Event::Message(Message {
                topic: message.topic().into(),
                data: serde_json::from_slice(message.payload()).unwrap(),
            })
        })
}

fn keys() -> impl Stream<Item = Event> {
    EventStream::new()
        .filter_map(|event| async move {
            match event.ok()? {
                CrossEvent::Key(key) if key.kind == KeyEventKind::Press => Some(key),
                _ => None,
            }
        })
        .filter_map(|key| async move {
            match key.code {
                KeyCode::Char('q') => Some(Event::Quit),
                KeyCode::Up => Some(Event::Up),
                KeyCode::Down => Some(Event::Down),
                _ => None,
            }
        })
}
