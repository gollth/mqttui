use ratatui::{
    layout::Constraint::{Fill, Length},
    prelude::*,
    widgets::{Block, HighlightSpacing, List, ListItem, Paragraph},
};

use crate::model::Model;

pub fn render(frame: &mut Frame, model: &mut Model) {
    let border = Block::bordered().title("MqtTUI");
    let area = border.inner(frame.area());
    frame.render_widget(border, frame.area());
    let [top, overview] = Layout::vertical([Length(3), Fill(0)]).areas(area);

    // Top header
    frame.render_widget(Paragraph::new(format!("Counter: {}", model.counter)), top);

    // Topic overview
    let list = List::new(model.topics().map(|topic| ListItem::from(topic.clone())))
        .highlight_style(Style::new())
        .highlight_symbol(">")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, overview, &mut model.state_topics);
}
