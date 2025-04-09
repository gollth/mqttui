use ratatui::{
    layout::Constraint::{Fill, Length},
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarState},
};

use crate::{
    jq::Jaqqer,
    model::{Filter, Mode, Model},
};

pub(crate) const SCROLL_BOTTOM_OFFSET: usize = 32;
pub(crate) const PROMPT: &str = "❯";

pub fn render(frame: &mut Frame, model: &Model) {
    let border = Block::bordered().title(Line::raw("MqtTUI").centered());
    let area = border.inner(frame.area());
    frame.render_widget(border, frame.area());
    match model.mode() {
        Mode::Topics { filter } => render_topics(frame, area, model, filter.as_ref()),
        Mode::Detail { topic, scroll, jq } => {
            render_details(frame, area, model, topic, jq, *scroll)
        }
    }
}

fn render_topics(frame: &mut Frame, area: Rect, model: &Model, filter: Option<&Filter>) {
    let [top, overview, prompt] = Layout::vertical([
        Length(1),
        Fill(0),
        Length(if filter.is_some() { 3 } else { 0 }),
    ])
    .areas(area);

    // Top header
    frame.render_widget(Paragraph::new(format!("Messages: {}", model.counter)), top);

    // Topic overview
    let list = List::new(model.topics().map(|(topic, message)| {
        let config = model.config();
        let style = if model.selection().is_some_and(|s| topic.as_str() == s) {
            let mut style = Style::new().bg(config.colors.selection).fg(Color::Black);
            if model.is_copy() {
                style = style.reversed();
            }
            style
        } else {
            Style::new().fg(message.freshness(config))
        };
        ListItem::new(message.topic.line(style)).style(style)
    }))
    .block(
        Block::new()
            .title(Line::raw("Topics").centered())
            .borders(Borders::TOP),
    );

    frame.render_widget(list, overview);

    if let Some(filter) = filter {
        let input = format!("{PROMPT} {}", filter.pattern());
        frame.render_widget(
            Paragraph::new(input.as_str()).block(
                Block::new()
                    .title(Line::raw(filter.kind()).centered())
                    .borders(Borders::TOP),
            ),
            prompt,
        );
        let x = (input.chars().count() as u16).min(prompt.width - 1);
        frame.set_cursor_position((prompt.x + x, prompt.y + 1));
    }
}

fn render_details(
    frame: &mut Frame,
    area: Rect,
    model: &Model,
    topic: &str,
    jq: &Jaqqer,
    scroll: u16,
) {
    let [header, pane, footer] = Layout::vertical([
        Length(2),
        Fill(0),
        Length(if jq.is_dormant() { 0 } else { 3 }),
    ])
    .areas(area);
    let [details, scroller] = Layout::horizontal([Fill(0), Length(1)]).areas(pane);

    // Top header with topic name
    frame.render_widget(
        Paragraph::new(
            Span::styled(topic, Style::new().fg(Color::Gray))
                .italic()
                .into_centered_line(),
        )
        .block(Block::new().borders(Borders::BOTTOM)),
        header,
    );

    let mut style = Style::new();
    if model.is_copy() {
        style = style.reversed();
    }

    let message = model.message(topic).unwrap_or_default();
    frame.render_widget(
        Paragraph::new(model.highlight(&message, details, scroll).style(style)).scroll((scroll, 0)),
        details,
    );
    frame.render_stateful_widget(
        Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        scroller,
        &mut ScrollbarState::new(message.lines().count().saturating_sub(SCROLL_BOTTOM_OFFSET))
            .position(scroll as usize),
    );

    let (input, style) = match jq {
        Jaqqer::Dormant => return,
        Jaqqer::Prompt(prompt) => (format!("{PROMPT} {prompt}"), Style::new().fg(Color::Blue)),
        Jaqqer::Active(prompt) => (
            format!("{PROMPT} {prompt}"),
            Style::new().fg(Color::LightRed),
        ),
    };

    frame.render_widget(
        Paragraph::new(vec![Line::raw(&input).style(style)]).block(
            Block::new()
                .title(Line::raw("JQ-Filter").centered())
                .borders(Borders::TOP),
        ),
        footer,
    );

    let x = (input.chars().count() as u16).min(footer.width - 1);
    frame.set_cursor_position((footer.x + x, footer.y + 1));
}
