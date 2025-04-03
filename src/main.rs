use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use mqttui::*;

fn main() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result?;
    Ok(())
}
fn run(terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame))?;
        if handle_events()? {
            break Ok(());
        }
    }
}

fn handle_events() -> std::io::Result<bool> {
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => return Ok(true),
            // handle other key events
            _ => {}
        },
        // handle other events
        _ => {}
    }
    Ok(false)
}
