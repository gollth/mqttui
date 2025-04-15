use std::pin::pin;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::KeyModifiers;
use crossterm::event::{Event as CrossEvent, EventStream, KeyCode, KeyEventKind};
use enum_as_inner::EnumAsInner;
use futures::{Stream, StreamExt, stream};
use rumqttc::{AsyncClient, ConnectionError, EventLoop, Incoming, QoS};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task;
use tokio::time::sleep;
use tracing::error;

const TICK: Duration = Duration::from_millis(100);

#[derive(Debug, EnumAsInner)]
pub enum Event {
    Render(RenderEvent),
    Update(UpdateEvent),
}
impl Event {
    pub fn is_disconnect(&self) -> bool {
        self.as_render()
            .is_some_and(|e| e == &RenderEvent::Disconnect)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RenderEvent {
    Tick,
    Up,
    Down,
    Left,
    Right,
    Back,
    Char(char),
    Backspace,
    Delete,
    Home,
    End,
    Select,
    Quit,
    Connect,
    Disconnect,
}

#[derive(Debug)]
pub enum UpdateEvent {
    Receive(rumqttc::Publish),
}

pub async fn start(
    client: AsyncClient,
    mut eventloop: EventLoop,
) -> Result<UnboundedReceiver<Event>> {
    let (tx, rx) = unbounded_channel();
    let tx2 = tx.clone();
    task::spawn(async move {
        loop {
            if eventloop.poll().await.is_err() {
                sleep(Duration::from_millis(500)).await;
                let _ = tx2.send(Event::Render(RenderEvent::Disconnect));
                continue;
            }

            let _ = client.subscribe("#", QoS::AtMostOnce).await;
            let _ = tx2.send(Event::Render(RenderEvent::Connect));
            loop {
                match eventloop.poll().await {
                    Err(ConnectionError::Io(_)) => {
                        let _ = tx2.send(Event::Render(RenderEvent::Disconnect));
                        break;
                    }
                    Err(other) => error!("Encountered unknown error: {other:#}"),
                    Ok(rumqttc::Event::Incoming(Incoming::Publish(message))) => {
                        let _ = tx2.send(Event::Update(UpdateEvent::Receive(message)));
                    }
                    _ => {}
                }
            }
        }
    });

    task::spawn(async move {
        let mut events = pin!(stream::select(keys(), tick()));
        while let Some(event) = events.next().await {
            let _ = tx.send(event);
        }
    });

    Ok(rx)
}

fn tick() -> impl Stream<Item = Event> {
    stream::unfold((), |_| async move {
        sleep(TICK).await;
        Some((Event::Render(RenderEvent::Tick), ()))
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
                KeyCode::Enter => Some(Event::Render(RenderEvent::Select)),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(Event::Render(RenderEvent::Quit))
                }
                KeyCode::Char(c) => Some(Event::Render(RenderEvent::Char(c))),
                KeyCode::Up => Some(Event::Render(RenderEvent::Up)),
                KeyCode::Down => Some(Event::Render(RenderEvent::Down)),
                KeyCode::Left => Some(Event::Render(RenderEvent::Left)),
                KeyCode::Right => Some(Event::Render(RenderEvent::Right)),
                KeyCode::Backspace => Some(Event::Render(RenderEvent::Backspace)),
                KeyCode::Delete => Some(Event::Render(RenderEvent::Delete)),
                KeyCode::Esc => Some(Event::Render(RenderEvent::Back)),
                KeyCode::Home => Some(Event::Render(RenderEvent::Home)),
                KeyCode::End => Some(Event::Render(RenderEvent::End)),
                _ => None,
            }
        })
}
