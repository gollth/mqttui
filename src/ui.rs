use ratatui::{
    Frame,
    widgets::{Block, Paragraph},
};

use crate::model::Model;

pub fn render(frame: &mut Frame, model: &Model) {
    let screen = frame.area();
    let border = Block::bordered().title("MQTTUI");

    frame.render_widget(
        Paragraph::new(format!("Counter: {}", model.counter)),
        border.inner(screen),
    );

    frame.render_widget(border, screen);
}
