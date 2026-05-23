use anyhow::Result;
use chrono::Local;
use ratatui::widgets::ListState;
use tui_textarea::TextArea;

use crate::db::Database;
use crate::task::Task;

#[derive(Copy, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Input,
    Confirm,
    EditNote,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Focus {
    Active,
    Done,
}

pub struct App {
    pub active: Vec<Task>,
    pub done: Vec<Task>,
    pub active_state: ListState,
    pub done_state: ListState,
    pub focus: Focus,
    pub mode: Mode,
    pub input: String,
    /// (id, title) of the task pending deletion confirmation.
    pub pending_delete: Option<(i64, String)>,
    pub note_editor: Option<TextArea<'static>>,
    db: Database,
}

impl App {
    pub fn new(db: Database) -> Result<Self> {
        let active = db.load_active()?;
        let done = db.load_done()?;
        let mut active_state = ListState::default();
        if !active.is_empty() {
            active_state.select(Some(0));
        }
        Ok(Self {
            active,
            done,
            active_state,
            done_state: ListState::default(),
            focus: Focus::Active,
            mode: Mode::Normal,
            input: String::new(),
            pending_delete: None,
            note_editor: None,
            db,
        })
    }

    pub fn switch_focus(&mut self) {
        match self.focus {
            Focus::Active => {
                if self.done_state.selected().is_none() && !self.done.is_empty() {
                    self.done_state.select(Some(0));
                }
                self.focus = Focus::Done;
            }
            Focus::Done => {
                if self.active_state.selected().is_none() && !self.active.is_empty() {
                    self.active_state.select(Some(0));
                }
                self.focus = Focus::Active;
            }
        }
    }

    pub fn move_selection_up(&mut self) {
        match self.focus {
            Focus::Active => {
                if let Some(i) = self.active_state.selected() {
                    if i > 0 {
                        self.active_state.select(Some(i - 1));
                    }
                }
            }
            Focus::Done => {
                if let Some(i) = self.done_state.selected() {
                    if i > 0 {
                        self.done_state.select(Some(i - 1));
                    }
                }
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        match self.focus {
            Focus::Active => {
                if self.active.is_empty() {
                    return;
                }
                let i = self
                    .active_state
                    .selected()
                    .map(|i| (i + 1).min(self.active.len() - 1))
                    .unwrap_or(0);
                self.active_state.select(Some(i));
            }
            Focus::Done => {
                if self.done.is_empty() {
                    return;
                }
                let i = self
                    .done_state
                    .selected()
                    .map(|i| (i + 1).min(self.done.len() - 1))
                    .unwrap_or(0);
                self.done_state.select(Some(i));
            }
        }
    }

    pub fn move_task_up(&mut self) -> Result<()> {
        if self.focus != Focus::Active {
            return Ok(());
        }
        let Some(i) = self.active_state.selected() else {
            return Ok(());
        };
        if i == 0 {
            return Ok(());
        }
        let (pos_a, id_a) = (self.active[i - 1].position, self.active[i - 1].id);
        let (pos_b, id_b) = (self.active[i].position, self.active[i].id);
        self.db.set_position(id_a, pos_b)?;
        self.db.set_position(id_b, pos_a)?;
        self.active[i - 1].position = pos_b;
        self.active[i].position = pos_a;
        self.active.swap(i - 1, i);
        self.active_state.select(Some(i - 1));
        Ok(())
    }

    pub fn move_task_down(&mut self) -> Result<()> {
        if self.focus != Focus::Active {
            return Ok(());
        }
        let Some(i) = self.active_state.selected() else {
            return Ok(());
        };
        if i + 1 >= self.active.len() {
            return Ok(());
        }
        let (pos_a, id_a) = (self.active[i].position, self.active[i].id);
        let (pos_b, id_b) = (self.active[i + 1].position, self.active[i + 1].id);
        self.db.set_position(id_a, pos_b)?;
        self.db.set_position(id_b, pos_a)?;
        self.active[i].position = pos_b;
        self.active[i + 1].position = pos_a;
        self.active.swap(i, i + 1);
        self.active_state.select(Some(i + 1));
        Ok(())
    }

    pub fn toggle_done(&mut self) -> Result<()> {
        match self.focus {
            Focus::Active => {
                let Some(i) = self.active_state.selected() else {
                    return Ok(());
                };
                let id = self.active[i].id;
                self.active[i].done = true;
                self.db.set_done(id, true)?;
                let task = self.active.remove(i);

                if self.active.is_empty() {
                    self.active_state.select(None);
                } else if i >= self.active.len() {
                    self.active_state.select(Some(self.active.len() - 1));
                } else {
                    self.active_state.select(Some(i));
                }

                let prev_sel = self.done_state.selected().map(|s| s + 1).unwrap_or(0);
                self.done.insert(0, task);
                self.done_state.select(Some(prev_sel));
            }
            Focus::Done => {
                let Some(i) = self.done_state.selected() else {
                    return Ok(());
                };
                let id = self.done[i].id;
                let new_pos = self.active.iter().map(|t| t.position).max().unwrap_or(0) + 1;
                self.done[i].done = false;
                self.done[i].position = new_pos;
                self.db.set_done(id, false)?;
                self.db.set_position(id, new_pos)?;
                let task = self.done.remove(i);

                if self.done.is_empty() {
                    self.done_state.select(None);
                } else if i >= self.done.len() {
                    self.done_state.select(Some(self.done.len() - 1));
                } else {
                    self.done_state.select(Some(i));
                }

                self.active.push(task);
                if self.active_state.selected().is_none() {
                    self.active_state.select(Some(self.active.len() - 1));
                }
            }
        }
        Ok(())
    }

    pub fn start_delete(&mut self) {
        let task = match self.focus {
            Focus::Active => self.active_state.selected().and_then(|i| self.active.get(i)),
            Focus::Done => self.done_state.selected().and_then(|i| self.done.get(i)),
        };
        if let Some(t) = task {
            self.pending_delete = Some((t.id, t.title.clone()));
            self.mode = Mode::Confirm;
        }
    }

    pub fn confirm_delete(&mut self) -> Result<()> {
        if let Some((id, _)) = self.pending_delete.take() {
            self.db.delete(id)?;
            if let Some(i) = self.active.iter().position(|t| t.id == id) {
                self.active.remove(i);
                if self.active.is_empty() {
                    self.active_state.select(None);
                } else if i >= self.active.len() {
                    self.active_state.select(Some(self.active.len() - 1));
                } else {
                    self.active_state.select(Some(i));
                }
            } else if let Some(i) = self.done.iter().position(|t| t.id == id) {
                self.done.remove(i);
                if self.done.is_empty() {
                    self.done_state.select(None);
                } else if i >= self.done.len() {
                    self.done_state.select(Some(self.done.len() - 1));
                } else {
                    self.done_state.select(Some(i));
                }
            }
        }
        self.mode = Mode::Normal;
        Ok(())
    }

    pub fn cancel_confirm(&mut self) {
        self.pending_delete = None;
        self.mode = Mode::Normal;
    }

    pub fn start_input(&mut self) {
        self.mode = Mode::Input;
        self.input.clear();
    }

    pub fn cancel_input(&mut self) {
        self.mode = Mode::Normal;
        self.input.clear();
    }

    pub fn confirm_input(&mut self) -> Result<()> {
        let title = self.input.trim().to_string();
        self.mode = Mode::Normal;
        self.input.clear();
        if title.is_empty() {
            return Ok(());
        }
        let next_pos = self.active.iter().map(|t| t.position).max().unwrap_or(0) + 1;
        let created_at = Local::now().format("%Y-%m-%d").to_string();
        let id = self.db.insert(&title, next_pos, &created_at)?;
        self.active.push(Task {
            id,
            title,
            position: next_pos,
            done: false,
            created_at,
            notes: String::new(),
        });
        self.active_state.select(Some(self.active.len() - 1));
        Ok(())
    }

    pub fn start_edit_note(&mut self) {
        let task = match self.focus {
            Focus::Active => self.active_state.selected().and_then(|i| self.active.get(i)),
            Focus::Done => self.done_state.selected().and_then(|i| self.done.get(i)),
        };
        let Some(task) = task else { return };

        let lines: Vec<String> = if task.notes.is_empty() {
            vec![String::new()]
        } else {
            task.notes.lines().map(|l| l.to_string()).collect()
        };

        let mut editor = TextArea::new(lines);
        editor.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(" Edit Notes  (Ctrl+S: save  Esc: cancel) "),
        );
        // Move cursor to end of content.
        editor.move_cursor(tui_textarea::CursorMove::Bottom);
        editor.move_cursor(tui_textarea::CursorMove::End);
        self.note_editor = Some(editor);
        self.mode = Mode::EditNote;
    }

    pub fn confirm_note(&mut self) -> Result<()> {
        if let Some(editor) = self.note_editor.take() {
            let notes = editor.lines().join("\n");
            let task_id = match self.focus {
                Focus::Active => self.active_state.selected().map(|i| self.active[i].id),
                Focus::Done => self.done_state.selected().map(|i| self.done[i].id),
            };
            if let Some(id) = task_id {
                self.db.set_notes(id, &notes)?;
                match self.focus {
                    Focus::Active => {
                        if let Some(i) = self.active_state.selected() {
                            self.active[i].notes = notes;
                        }
                    }
                    Focus::Done => {
                        if let Some(i) = self.done_state.selected() {
                            self.done[i].notes = notes;
                        }
                    }
                }
            }
        }
        self.mode = Mode::Normal;
        Ok(())
    }

    pub fn cancel_note(&mut self) {
        self.note_editor = None;
        self.mode = Mode::Normal;
    }

    /// Returns the notes of the currently focused and selected task.
    pub fn selected_notes(&self) -> &str {
        match self.focus {
            Focus::Active => self
                .active_state
                .selected()
                .and_then(|i| self.active.get(i))
                .map(|t| t.notes.as_str())
                .unwrap_or(""),
            Focus::Done => self
                .done_state
                .selected()
                .and_then(|i| self.done.get(i))
                .map(|t| t.notes.as_str())
                .unwrap_or(""),
        }
    }
}
