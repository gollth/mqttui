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
    /// Until which time since last receptions topics are considered "fresh"
    #[serde(with = "humantime_serde")]
    #[derivative(Default(value = "Duration::from_millis(500)"))]
    pub fresh_until: Duration,

    /// From which time on since last receptions topics are considered "stale"
    #[serde(with = "humantime_serde")]
    #[derivative(Default(value = "Duration::from_secs(5)"))]
    pub stale_after: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct ColorConfig {
    /// How to color the selection
    #[derivative(Default(value = "Color::White"))]
    pub selection: Color,

    /// How to color retain topics
    #[derivative(Default(value = "Color::Cyan"))]
    pub retain: Color,

    /// How to color fresh topics
    #[derivative(Default(value = "Color::White"))]
    pub fresh: Color,

    /// How to color non-fresh but also non-stale topics
    #[derivative(Default(value = "Color::Gray"))]
    pub intime: Color,

    /// How to color stale topics
    #[derivative(Default(value = "Color::DarkGray"))]
    pub stale: Color,
}

#[derive(Clone, Debug, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct KeyConfig {
    /// Key to use for searching in topics
    #[derivative(Default(value = "'/'"))]
    pub search: char,

    /// Key to use for negative searching in topics
    #[derivative(Default(value = "'?'"))]
    pub ignore: char,

    /// Key to used to copy to clipboard
    #[derivative(Default(value = "'y'"))]
    pub copy: char,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let name = env!("CARGO_PKG_NAME");
        let dir = xdg::BaseDirectories::with_prefix(name)
            .context("failed to read XDG config directory")?;
        Ok(dir.place_config_file("config.toml")?)
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
