use ratatui::{Frame, widgets::Block};

pub fn render(frame: &mut Frame) {
    let screen = frame.area();

    frame.render_widget(Block::bordered().title("MQTTUI"), screen);
}
