use std::pin::pin;
use std::time::Duration;

use crossterm::event::{Event as CrossEvent, EventStream, KeyCode, KeyEventKind};
use futures::{Stream, StreamExt, stream};
use paho_mqtt::{AsyncClient, QoS};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task;
use tokio::time::sleep;

use crate::model::{Event, RenderEvent, UpdateEvent};

const TICK: Duration = Duration::from_millis(100);

pub async fn handler(client: &mut AsyncClient) -> UnboundedReceiver<Event> {
    let (tx, rx) = unbounded_channel();
    client.subscribe("#", QoS::default());

    let stream = client.get_stream(None);
    task::spawn(async move {
        let events = stream::select(keys(), messages(stream));
        let mut events = pin!(stream::select(events, tick()));
        while let Some(event) = events.next().await {
            let _ = tx.send(event);
        }
    });
    rx
}

fn tick() -> impl Stream<Item = Event> {
    stream::unfold((), |_| async move {
        sleep(TICK).await;
        Some((Event::Render(RenderEvent::Tick), ()))
    })
}

fn messages(stream: impl Stream<Item = Option<paho_mqtt::Message>>) -> impl Stream<Item = Event> {
    stream
        .filter_map(|message| async move { message })
        .map(|message| Event::Update(UpdateEvent::Receive(message.into())))
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
                KeyCode::Char(c) => Some(Event::Render(RenderEvent::Char(c))),
                KeyCode::Up => Some(Event::Render(RenderEvent::Up)),
                KeyCode::Down => Some(Event::Render(RenderEvent::Down)),
                KeyCode::Esc => Some(Event::Render(RenderEvent::Back)),
                _ => None,
            }
        })
}
