mod app;
mod db;
mod task;
mod ui;

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::{App, Mode};
use db::Database;

fn main() -> Result<()> {
    let db = Database::open(&db_path())?;
    let mut app = App::new(db)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;

    result
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        match event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    return Ok(());
                }
                match app.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('n') => app.start_input(),
                        KeyCode::Char('r') => app.start_edit_title(),
                        KeyCode::Char('t') => app.start_edit_tags(),
                        KeyCode::Char('/') => app.start_search(),
                        KeyCode::Char('z')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.undo()?;
                        }
                        KeyCode::Esc => app.clear_filter(),
                        KeyCode::Tab => app.switch_focus(),
                        KeyCode::Down | KeyCode::Char('j') => app.move_selection_down(),
                        KeyCode::Up | KeyCode::Char('k') => app.move_selection_up(),
                        KeyCode::Char('J') => app.move_task_down()?,
                        KeyCode::Char('K') => app.move_task_up()?,
                        KeyCode::Char(' ') => app.toggle_done()?,
                        KeyCode::Char('d') => app.start_delete(),
                        KeyCode::Char('e') => app.start_edit_note(),
                        _ => {}
                    },
                    Mode::Confirm => match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => app.confirm_delete()?,
                        KeyCode::Char('n') | KeyCode::Esc => app.cancel_confirm(),
                        _ => {}
                    },
                    Mode::Input => match key.code {
                        KeyCode::Enter => app.confirm_input()?,
                        KeyCode::Esc => app.cancel_input(),
                        KeyCode::Char(c) => app.input.push(c),
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    Mode::EditTitle => match key.code {
                        KeyCode::Enter => app.confirm_edit_title()?,
                        KeyCode::Esc => app.cancel_edit_title(),
                        KeyCode::Char(c) => app.input.push(c),
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    Mode::EditTags => match key.code {
                        KeyCode::Enter => app.confirm_edit_tags()?,
                        KeyCode::Esc => app.cancel_edit_tags(),
                        KeyCode::Char(c) => app.input.push(c),
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    Mode::Search => match key.code {
                        KeyCode::Enter => app.apply_search(),
                        KeyCode::Esc => app.cancel_search(),
                        KeyCode::Char(c) => app.search_push(c),
                        KeyCode::Backspace => app.search_pop(),
                        _ => {}
                    },
                    Mode::EditNote => {
                        if key.code == KeyCode::Char('s')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            app.confirm_note()?;
                        } else if key.code == KeyCode::Esc {
                            app.cancel_note();
                        } else if let Some(editor) = app.note_editor.as_mut() {
                            editor.input(key);
                        }
                    }
                }
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn db_path() -> String {
    if let Some(data_dir) = dirs::data_dir() {
        let dir = data_dir.join("prioritize");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir.join("tasks.db").to_string_lossy().into_owned();
        }
    }
    "tasks.db".to_string()
}
