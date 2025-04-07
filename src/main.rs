use color_eyre::{Result, eyre::eyre};
use mqttui::*;
use mqttui::{events::Event, model::Model};
use paho_mqtt::{AsyncClient, ConnectOptions, CreateOptionsBuilder};
use ratatui::{Terminal, prelude::Backend};
use tokio::sync::mpsc::UnboundedReceiver;

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = init().await?;
    let rx = events::handler(&mut client).await;

    let mut terminal = ratatui::init();
    // use ratatui::backend::TestBackend;
    // let mut terminal = Terminal::new(TestBackend::new(10, 10)).unwrap();

    let result = run(&mut terminal, rx).await;
    ratatui::restore();
    result?;
    Ok(())
}

async fn init() -> Result<AsyncClient> {
    color_eyre::install()?;

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        hook(panic_info);
    }));

    let client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri("localhost:1883")
            .client_id("foo")
            .finalize(),
    )?;
    client.connect(ConnectOptions::default()).await?;
    Ok(client)
}

async fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    mut events: UnboundedReceiver<Event>,
) -> Result<()> {
    let mut model = Model::new()?;
    while !model.shutdown {
        let event = events.recv().await.ok_or(eyre!("runtime died"))?;
        let rendering = event.is_render();

        model.update(event);
        // eprintln!("{model:#?}");

        if rendering {
            terminal.draw(|frame| ui::render(frame, &model))?;
        }
    }
    Ok(())
}
