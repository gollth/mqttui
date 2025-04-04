use ratatui::{
    layout::Constraint::{Fill, Length},
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::model::Model;

pub fn render(frame: &mut Frame, model: &mut Model) {
    let border = Block::bordered().title(Line::raw("MqtTUI").centered());
    let area = border.inner(frame.area());
    frame.render_widget(border, frame.area());
    let [top, overview] = Layout::vertical([Length(1), Fill(0)]).areas(area);

    // Top header
    frame.render_widget(Paragraph::new(format!("Messages: {}", model.counter)), top);

    // Topic overview
    let list = List::new(model.topics().map(|message| {
        ListItem::new(message.topic.clone()).style(Style::default().fg(message.freshness()))
    }))
    .highlight_style(Style::new().bg(Color::White).fg(Color::Black))
    .block(
        Block::new()
            .title(Line::raw("Topics").centered())
            .borders(Borders::TOP),
    );

    frame.render_stateful_widget(list, overview, &mut model.state_topics);

    if model.popup() {
        frame.render_widget(
            CopyPopup,
            Rect {
                x: area.width / 2 - 5,
                y: area.height.saturating_sub(8),
                width: 10,
                height: 3,
            },
        );
    }
}

#[derive(Debug, Default)]
struct CopyPopup;

impl Widget for CopyPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        Paragraph::new(" Copied")
            .block(Block::new().borders(Borders::ALL))
            .style(Style::new().green())
            .render(area, buf);
    }
}
