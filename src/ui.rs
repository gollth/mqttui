use ratatui::{
    layout::Constraint::{Fill, Length},
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::model::{Filter, Mode, Model};

pub fn render(frame: &mut Frame, model: &Model) {
    let border = Block::bordered().title(Line::raw("MqtTUI").centered());
    let area = border.inner(frame.area());
    frame.render_widget(border, frame.area());
    match model.mode() {
        Mode::Topics { filter } => render_topics(frame, area, model, filter.as_ref()),
        Mode::Detail { topic } => render_details(frame, area, model, topic),
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
            if model.highlight_copy() {
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
        frame.render_widget(
            Paragraph::new(format!(">> {}", filter.pattern())).block(
                Block::new()
                    .title(Line::raw(filter.kind()).centered())
                    .borders(Borders::TOP),
            ),
            prompt,
        )
    }
}

fn render_details(frame: &mut Frame, area: Rect, model: &Model, topic: &str) {
    let [header, pane] = Layout::vertical([Length(1), Fill(0)]).areas(area);

    // Top header with topic name
    frame.render_widget(
        Paragraph::new(
            Span::styled(topic, Style::new().fg(Color::Gray))
                .italic()
                .into_centered_line(),
        ),
        header,
    );

    frame.render_widget(
        Paragraph::new(Text::from_iter(
            model
                .message(topic)
                .iter()
                .flat_map(|message| message.lines())
                .map(Line::raw),
        ))
        .block(
            Block::new()
                .title(Line::raw("Message"))
                .borders(Borders::TOP),
        ),
        pane,
    );
}
