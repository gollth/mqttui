use crossterm::event::{self, KeyCode, KeyEventKind};
use model::{Event, Model};
use mqttui::*;

fn main() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result?;
    Ok(())
}
fn run(terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
    let mut model = Model::default();
    while !model.shutdown {
        terminal.draw(|frame| ui::render(frame, &model))?;

        let mut event = handle()?;
        while let Some(e) = event {
            event = model::update(&mut model, e);
        }
    }
    Ok(())
}

fn handle() -> anyhow::Result<Option<Event>> {
    match event::read()? {
        crossterm::event::Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => return Ok(Some(Event::Quit)),
            KeyCode::Up => return Ok(Some(Event::Up)),
            KeyCode::Down => return Ok(Some(Event::Down)),
            _ => {}
        },
        _ => {}
    }
    Ok(None)
}
