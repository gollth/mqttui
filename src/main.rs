use color_eyre::{Result, eyre};
use crossterm::event::{Event as CrossEvent, EventStream, KeyCode, KeyEventKind};
use futures::{FutureExt, StreamExt};
use model::{Event, Model};
use mqttui::*;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        hook(panic_info);
    }));
    let mut terminal = ratatui::init();
    let result = run(&mut terminal).await;
    ratatui::restore();
    result?;
    Ok(())
}

async fn run(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut model = Model::default();
    let mut events = EventHandler::new();
    let _client = mqtt::Client::new("localhost", events.sender()).await?;
    while !model.shutdown {
        terminal.draw(|frame| ui::render(frame, &model))?;

        let mut event = Some(events.next().await?);
        while let Some(e) = event {
            event = model::update(&mut model, e);
        }
    }
    Ok(())
}

struct EventHandler {
    rx: UnboundedReceiver<Event>,
    tx: UnboundedSender<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        tokio::spawn(Self::run(tx.clone()));
        Self { rx, tx }
    }

    pub fn sender(&self) -> UnboundedSender<Event> {
        self.tx.clone()
    }

    pub async fn next(&mut self) -> Result<Event> {
        self.rx
            .recv()
            .await
            .ok_or(eyre::Report::msg("Async runtime died"))
    }

    async fn run(tx: UnboundedSender<Event>) -> Result<()> {
        let mut stream = EventStream::new();
        while let Some(Ok(CrossEvent::Key(key))) = stream.next().fuse().await {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') => tx.send(Event::Quit)?,
                KeyCode::Up => tx.send(Event::Up)?,
                KeyCode::Down => tx.send(Event::Down)?,
                _ => {}
            }
        }
        Err(eyre::Report::msg("could not read next event"))
    }
}
