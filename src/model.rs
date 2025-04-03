#[derive(Default, Debug)]
pub struct Model {
    pub shutdown: bool,
    pub counter: i32,
}

pub enum Event {
    Tick,
    Up,
    Down,
    Quit,
    Message { topic: String, data: String },
}

pub fn update(state: &mut Model, event: Event) -> Option<Event> {
    match event {
        Event::Quit => state.shutdown = true,
        Event::Up => state.counter += 1,
        Event::Down => state.counter -= 1,
        Event::Tick => {}
        Event::Message { .. } => state.counter += 1,
    }
    None
}
