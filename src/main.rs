use std::fs::File;

use clap::Parser;
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use mqttui::{
    config::Config,
    events::{self},
    model::Model,
    ui,
};
use petname::petname;
use ratatui::{Terminal, prelude::Backend};
use rumqttc::{AsyncClient, EventLoop, MqttOptions};
use tracing::info;
use tracing_subscriber::{
    Layer, filter::filter_fn, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};
use url::Url;

/// Mqtt TUI
///
/// Search through and inspect contents of MQTT topics
#[derive(Debug, Parser)]
struct Args {
    /// The URL of the MQTT broker to connect to
    #[arg(default_value = "mqtt://localhost:1883", value_parser = validate_url)]
    broker: Url,

    /// Immediately quit, when the connection to the broker is lost
    ///
    /// By default the TUI will stay open and try to automatically reconnect when the broker comes
    /// back online
    #[arg(short('q'), long)]
    quit: bool,

    /// Print the path of where the config file is read from
    #[arg(long)]
    print_config_path: bool,

    /// Print the path of where the log file is placed
    #[arg(long)]
    print_log_path: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    if args.print_config_path {
        println!("{}", Config::path()?.display());
        return Ok(());
    }

    if args.print_log_path {
        println!("{}", Config::log()?.display());
        return Ok(());
    }

    let mut terminal = ratatui::init();
    let result = run(args.broker, &mut terminal, !args.quit).await;

    ratatui::restore();
    result?;
    Ok(())
}

async fn init(broker: &Url) -> Result<(AsyncClient, EventLoop)> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        hook(panic_info);
    }));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_span_events(FmtSpan::NEW)
                .compact()
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_writer(File::create(Config::log()?)?)
                // rumqttc is pretty verbose, ignore it
                .with_filter(filter_fn(|meta| meta.target() != "rumqttc")),
        )
        .init();
    info!("Started MqtTUI: {broker}");

    let name = env!("CARGO_PKG_NAME");
    let name = format!("{name}-{}", petname(2, "-").unwrap());

    let mut options = MqttOptions::new(
        name,
        broker
            .host_str()
            .ok_or(eyre!("Failed to find out host name from broker URL"))
            .wrap_err(broker.clone())?,
        broker.port_or_known_default().unwrap_or(1883),
    );
    options.set_max_packet_size(1000000, 1024);
    let (client, eventloop) = AsyncClient::new(options, 10);
    Ok((client, eventloop))
}

async fn run<B: Backend>(broker: Url, terminal: &mut Terminal<B>, reconnect: bool) -> Result<()> {
    let (client, eventloop) = init(&broker).await?;
    let mut events = events::start(client, eventloop).await?;
    let mut model = Model::new(broker.clone())?;
    while !model.shutdown {
        let event = events.recv().await.ok_or(eyre!("runtime died"))?;
        let rendering = event.is_render();

        if !reconnect && event.is_disconnect() {
            return Err(eyre!(broker)).wrap_err("Connection to broker lost");
        }

        model.update(event);

        if rendering {
            terminal.draw(|frame| ui::render(frame, &model))?;
        }
    }
    Ok(())
}

fn validate_url(arg: &str) -> Result<Url, url::ParseError> {
    let url = Url::parse(arg).or_else(|_| format!("{arg}:1883").parse())?;
    if url.host().is_none() {
        return Url::parse(&format!("mqtt://{arg}")).or_else(|_| format!("{arg}:1883").parse());
    }
    Ok(url)
}
