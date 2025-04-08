use color_eyre::{Result, eyre::eyre};
use ratatui::{
    style::{Color, Style},
    text::Line,
};
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};
use syntect_tui::into_span;

use crate::config::Config;

pub struct Highlighter {
    syntaxes: SyntaxSet,
    themes: ThemeSet,
    theme: String,
}

impl Highlighter {
    pub(crate) fn new(config: &Config) -> Result<Self> {
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = config.colors.theme.clone();
        if !themes.themes.contains_key(&theme) {
            return Err(eyre!(
                "Color theme '{theme}' is not supported. Choose between: {:#?}",
                themes.themes.keys().collect::<Vec<_>>()
            ));
        }

        Ok(Self {
            syntaxes,
            themes,
            theme,
        })
    }

    pub(crate) fn highlight<'a>(&self, line: &'a str) -> Line<'a> {
        let syntax = self.syntaxes.find_syntax_by_name("JSON").unwrap();
        let mut colorer = HighlightLines::new(syntax, &self.themes.themes[&self.theme]);
        colorer
            .highlight_line(line, &self.syntaxes)
            .unwrap()
            .into_iter()
            .filter_map(|span| into_span(span).ok())
            .map(|span| span.patch_style(Style::new().bg(Color::Reset)))
            .collect()
    }
}
