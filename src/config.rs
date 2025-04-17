use std::{path::PathBuf, time::Duration};

use color_eyre::{Result, eyre::Context};
use derivative::Derivative;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub topics: TopicsConfig,

    #[serde(default)]
    pub colors: ColorConfig,

    #[serde(default)]
    pub keys: KeyConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct TopicsConfig {
    /// How many message to keep for each topic. 0 for all
    #[serde(default)]
    pub buffer_size: usize,

    /// Until which time since last receptions topics are considered "fresh"
    #[serde(default = "defaults::topics::fresh_until", with = "humantime_serde")]
    #[derivative(Default(value = "defaults::topics::fresh_until()"))]
    pub fresh_until: Duration,

    /// From which time on since last receptions topics are considered "stale"
    #[serde(default = "defaults::topics::stale_after", with = "humantime_serde")]
    #[derivative(Default(value = "defaults::topics::stale_after()"))]
    pub stale_after: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct ColorConfig {
    /// Color theme of JSON syntax highlighter
    #[serde(default = "defaults::colors::theme")]
    #[derivative(Default(value = "defaults::colors::theme()"))]
    pub theme: String,

    /// How to color the selection
    #[serde(default = "defaults::colors::selection")]
    #[derivative(Default(value = "defaults::colors::selection()"))]
    pub selection: Color,

    /// How to color retain topics
    #[serde(default = "defaults::colors::retain")]
    #[derivative(Default(value = "defaults::colors::retain()"))]
    pub retain: Color,

    /// How to color fresh topics
    #[serde(default = "defaults::colors::fresh")]
    #[derivative(Default(value = "defaults::colors::fresh()"))]
    pub fresh: Color,

    /// How to color non-fresh but also non-stale topics
    #[serde(default = "defaults::colors::intime")]
    #[derivative(Default(value = "defaults::colors::intime()"))]
    pub intime: Color,

    /// How to color stale topics
    #[serde(default = "defaults::colors::stale")]
    #[derivative(Default(value = "defaults::colors::stale()"))]
    pub stale: Color,
}

#[derive(Clone, Debug, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct KeyConfig {
    /// Key to use for searching in topics
    #[serde(default = "defaults::keys::search")]
    #[derivative(Default(value = "defaults::keys::search()"))]
    pub search: char,

    /// Key to use for negative searching in topics
    #[serde(default = "defaults::keys::ignore")]
    #[derivative(Default(value = "defaults::keys::ignore()"))]
    pub ignore: char,

    /// Key to used to copy to clipboard
    #[serde(default = "defaults::keys::copy")]
    #[derivative(Default(value = "defaults::keys::copy()"))]
    pub copy: char,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let name = env!("CARGO_PKG_NAME");
        let dir = xdg::BaseDirectories::with_prefix(name)
            .context("failed to read XDG config directory")?;
        Ok(dir.place_config_file("config.toml")?)
    }

    pub fn log() -> Result<PathBuf> {
        let name = env!("CARGO_PKG_NAME");
        let path = xdg::BaseDirectories::with_prefix(name)
            .context("failed to read XDG config directory")?
            .place_cache_file(format!("{name}.log"))?;
        Ok(path)
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;

        if !path.exists() {
            std::fs::write(
                &path,
                toml::to_string_pretty(&Self::default())
                    .context("failed to serialize config to TOML")?,
            )
            .context("failed to write config file to XDG config dir")?;
        }

        let content = std::fs::read_to_string(&path).context("failed to read config file")?;
        toml::from_str(&content)
            .context(path.display().to_string())
            .context("failed to parse config")
    }
}

pub(crate) mod defaults {
    use super::*;

    pub(crate) mod topics {
        use super::*;

        pub(crate) fn fresh_until() -> Duration {
            Duration::from_millis(500)
        }

        pub(crate) fn stale_after() -> Duration {
            Duration::from_secs(5)
        }
    }

    pub(crate) mod colors {
        use super::*;

        pub(crate) fn theme() -> String {
            "Solarized (dark)".into()
        }

        pub(crate) fn selection() -> Color {
            Color::White
        }

        pub(crate) fn retain() -> Color {
            Color::Cyan
        }

        pub(crate) fn fresh() -> Color {
            Color::White
        }

        pub(crate) fn intime() -> Color {
            Color::Gray
        }

        pub(crate) fn stale() -> Color {
            Color::DarkGray
        }
    }

    pub(crate) mod keys {
        pub(crate) fn search() -> char {
            '/'
        }

        pub(crate) fn ignore() -> char {
            '?'
        }

        pub(crate) fn copy() -> char {
            'y'
        }
    }
}
