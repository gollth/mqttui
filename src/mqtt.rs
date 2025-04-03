use color_eyre::Result;
use rumqttc::{AsyncClient, Incoming, MqttOptions, QoS};
use tokio::{
    sync::mpsc::UnboundedSender,
    task::{self, JoinHandle},
};

use crate::model::Event;

pub struct Client {
    inner: AsyncClient,
    handle: JoinHandle<Result<()>>,
}

impl Client {
    pub async fn new(host: &str, tx: UnboundedSender<Event>) -> Result<Self> {
        let name = env!("CARGO_PKG_NAME");
        let (inner, mut connection) = AsyncClient::new(MqttOptions::new(name, host, 1883), 10);

        inner
            .subscribe("fleet/v1/fleet_visualization/orders", QoS::AtMostOnce)
            .await?;

        let handle = task::spawn(async move {
            loop {
                if let rumqttc::Event::Incoming(Incoming::Publish(message)) =
                    connection.poll().await?
                {
                    // eprintln!("{message:?}");
                    tx.send(Event::Message {
                        topic: message.topic,
                        data: String::from_utf8(message.payload.into())?,
                    })?;
                }
            }
        });

        Ok(Self { inner, handle })
    }
}
