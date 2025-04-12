use clap::Parser;
use color_eyre::{Result, eyre::eyre};
use mqttui::{
    config::Config,
    events::{self},
    model::Model,
    ui,
};
use petname::petname;
use ratatui::{Terminal, prelude::Backend};
use rumqttc::{AsyncClient, EventLoop, MqttOptions};
use url::Url;

/// Mqtt TUI
///
/// Search through and inspect contents of MQTT topics
#[derive(Debug, Parser)]
struct Args {
    /// The URL of the MQTT broker to connect to
    #[arg(short('b'), long, default_value = "mqtt://localhost:1883", value_parser = validate_url)]
    broker: Url,

    /// Print the path of the default config
    #[arg(long)]
    print_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    if args.print_config {
        println!("{}", Config::path()?.display());
        return Ok(());
    }

    let mut terminal = ratatui::init();
    // use ratatui::backend::TestBackend;
    // let mut terminal = Terminal::new(TestBackend::new(10, 10)).unwrap();

    let result = run(args.broker, &mut terminal).await;
    ratatui::restore();
    result?;
    Ok(())
}

async fn init(broker: &Url) -> (AsyncClient, EventLoop) {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        hook(panic_info);
    }));

    let name = env!("CARGO_PKG_NAME");
    let name = format!("{name}-{}", petname(2, "-").unwrap());

    let mut options = MqttOptions::new(
        name,
        broker.host_str().unwrap(),
        broker.port_or_known_default().unwrap_or(1883),
    );
    options.set_max_packet_size(1000000, 1024);
    let (client, eventloop) = AsyncClient::new(options, 10);
    (client, eventloop)
}

async fn run<B: Backend>(broker: Url, terminal: &mut Terminal<B>) -> Result<()> {
    let (client, eventloop) = init(&broker).await;
    let mut events = events::start_handler(client, eventloop).await?;
    let mut model = Model::new(broker)?;
    while !model.shutdown {
        let event = events.recv().await.ok_or(eyre!("runtime died"))?;
        let rendering = event.is_render();

        model.update(event);

        if rendering {
            terminal.draw(|frame| ui::render(frame, &model))?;
        }
    }
    Ok(())
}

fn validate_url(arg: &str) -> Result<Url, url::ParseError> {
    Url::parse(arg).or_else(|_| format!("{arg}:1883").parse())
}
