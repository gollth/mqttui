use clap::Parser;
use color_eyre::eyre::Context;
use color_eyre::{Result, eyre::eyre};
use mqttui::*;
use mqttui::{events::Event, model::Model};
use paho_mqtt::{AsyncClient, ConnectOptions, CreateOptionsBuilder};
use petname::petname;
use ratatui::{Terminal, prelude::Backend};
use tokio::sync::mpsc::UnboundedReceiver;
use url::Url;

/// Mqtt TUI
///
/// Search through and inspect contents of MQTT topics
#[derive(Debug, Parser)]
struct Args {
    /// The URL of the MQTT broker to connect to
    #[arg(short('b'), long, default_value = "mqtt://localhost:1883", value_parser = validate_url)]
    broker: Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let mut client = init(args.broker).await?;
    let rx = events::handler(&mut client).await;

    let mut terminal = ratatui::init();
    // use ratatui::backend::TestBackend;
    // let mut terminal = Terminal::new(TestBackend::new(10, 10)).unwrap();

    let result = run(&mut terminal, rx).await;
    ratatui::restore();
    result?;
    Ok(())
}

async fn init(broker: Url) -> Result<AsyncClient> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        hook(panic_info);
    }));

    let client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(broker.clone())
            .client_id(format!("mqttui-{}", petname(2, "-").unwrap()))
            .finalize(),
    )?;
    client
        .connect(ConnectOptions::default())
        .await
        .context(broker)
        .context("Failed to connect to MQTT broker")?;
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

fn validate_url(arg: &str) -> Result<Url, url::ParseError> {
    Url::parse(arg).or_else(|_| format!("{arg}:1883").parse())
}
