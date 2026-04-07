use std::{
    collections::VecDeque,
    fmt::Display,
    io::Write,
    iter::once,
    process::{Command, Stdio},
    time::Instant,
};

use color_eyre::eyre::{Context, eyre};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::{Config, ProtocolConfig},
    model::Topic,
};

/// One single message received on one topic
#[derive(Clone, Debug)]
pub struct Message {
    pub(crate) topic: Topic,
    pub(crate) data: Result<Value, String>,
    pub(crate) format: Format,
    pub(crate) text: String,
    pub(crate) retain: bool,
    pub(crate) last: Instant,
}

/// History of message received on one topic
pub struct Messages {
    pub(crate) topic: Topic,
    pub(crate) latest: Message,
    pub(crate) queue: VecDeque<Message>,
}

#[derive(Clone, Default, Serialize, Deserialize, Derivative)]
#[derivative(Debug)]
pub enum Format {
    #[default]
    Unknown,
    Json,
    Cbor,
    Custom(ProtocolConfig),
}

impl Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "???"),
            Self::Json => write!(f, "JSON"),
            Self::Cbor => write!(f, "CBOR"),
            Self::Custom(proto) => write!(f, "{}", proto.label),
        }
    }
}

fn execute(exe: &ProtocolConfig, binary: &[u8]) -> color_eyre::Result<Value> {
    let label = &exe.label;
    let process = Command::new(&exe.program)
        .args(&exe.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err(format!("{label}: cannot call program"))?;

    process
        .stdin
        .as_ref()
        .ok_or(eyre!("{label}: cannot pipe data into program"))?
        .write_all(binary)
        .wrap_err(format!("{label}: cannot pip data into program"))?;

    let output = process
        .wait_with_output()
        .wrap_err(format!("{label}: failed to wait for process"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("{label}: {message}"));
    }

    let value = serde_json::from_slice(&output.stdout)
        .wrap_err(format!("{label}: did not produce not valid JSON"))?;
    Ok(value)
}

impl Format {
    fn interprete(&self, message: &rumqttc::Publish) -> Option<Message> {
        let value = match self {
            Self::Unknown => Err("Message does not follow any known protocol".to_string()),
            Self::Json => Ok(serde_json::from_str(str::from_utf8(&message.payload).ok()?).ok()?),
            Self::Cbor => Ok(ciborium::from_reader(message.payload.as_ref()).ok()?),
            Self::Custom(exe) if exe.topic.is_match(&message.topic) => {
                execute(exe, &message.payload)
                    .inspect_err(|error| {
                        tracing::error!(
                            label = exe.label,
                            exe = exe.program,
                            args = ?exe.args,
                            %error,
                            "Failed to interprete custom protocol"
                        )
                    })
                    .map_err(|e| e.to_string())
            }
            _ => return None,
        };
        Some(Message {
            topic: Topic::new(&message.topic),
            format: self.clone(),
            retain: message.retain,
            last: Instant::now(),
            text: value
                .as_ref()
                .ok()
                .and_then(|v| serde_json::to_string_pretty(v).ok())
                .or_else(|| String::from_utf8(message.payload.to_vec()).ok())
                .unwrap_or_else(|| "<binary>".into()),
            data: value,
        })
    }
}

pub(crate) struct Protocols(Vec<Format>);

impl Protocols {
    pub(crate) fn new(config: &Config) -> Self {
        Self(
            config
                .protocols
                .iter()
                .cloned()
                .map(Format::Custom)
                .chain(once(Format::Json))
                .chain(once(Format::Cbor))
                .chain(once(Format::Unknown))
                .collect(),
        )
    }

    pub(crate) fn interprete(&self, message: rumqttc::Publish) -> Message {
        self.0
            .iter()
            .find_map(|proto| proto.interprete(&message))
            .expect("Last protocol to be `Unknown` which always succeeds")
    }
}
