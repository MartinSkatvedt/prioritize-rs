use std::collections::VecDeque;

use anyhow::Result;
use chrono::Local;
use ratatui::widgets::ListState;
use tui_textarea::TextArea;

use crate::db::Database;
use crate::task::Task;

const UNDO_LIMIT: usize = 20;

#[derive(Copy, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Input,
    Confirm,
    EditNote,
    Search,
    EditTitle,
    EditTags,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Focus {
    Active,
    Done,
}

pub enum UndoAction {
    DeleteTask { task: Task, was_done: bool },
    ToggleDone { id: i64, from_done: bool },
    EditTitle { id: i64, old_title: String },
    EditTags { id: i64, old_tags: Vec<String> },
    EditNotes { id: i64, old_notes: String },
    Reorder { id_a: i64, pos_a: i64, id_b: i64, pos_b: i64 },
}

pub struct App {
    pub active: Vec<Task>,
    pub done: Vec<Task>,
    pub active_state: ListState,
    pub done_state: ListState,
    pub focus: Focus,
    pub mode: Mode,
    /// Shared input buffer for: new task, rename, tags, search.
    pub input: String,
    pub filter: String,
    pub pending_delete: Option<(i64, String)>,
    pub note_editor: Option<TextArea<'static>>,
    undo_stack: VecDeque<UndoAction>,
    db: Database,
}

// ─── construction ────────────────────────────────────────────────────────────

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
            filter: String::new(),
            pending_delete: None,
            note_editor: None,
            undo_stack: VecDeque::new(),
            db,
        })
    }
}

// ─── filter / search helpers ─────────────────────────────────────────────────

impl App {
    fn task_matches(&self, task: &Task) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let q = self.filter.to_lowercase();
        task.title.to_lowercase().contains(&q)
            || task.notes.to_lowercase().contains(&q)
            || task.tags.iter().any(|t| t.to_lowercase().contains(&q))
    }

    /// Real (vec) indices of active tasks that pass the current filter.
    pub fn filtered_active_indices(&self) -> Vec<usize> {
        self.active
            .iter()
            .enumerate()
            .filter(|(_, t)| self.task_matches(t))
            .map(|(i, _)| i)
            .collect()
    }

    /// Real (vec) indices of done tasks that pass the current filter.
    pub fn filtered_done_indices(&self) -> Vec<usize> {
        self.done
            .iter()
            .enumerate()
            .filter(|(_, t)| self.task_matches(t))
            .map(|(i, _)| i)
            .collect()
    }

    /// Maps the currently selected *filtered* index → real active vec index.
    pub fn active_real_idx(&self) -> Option<usize> {
        let sel = self.active_state.selected()?;
        self.filtered_active_indices().get(sel).copied()
    }

    /// Maps the currently selected *filtered* index → real done vec index.
    pub fn done_real_idx(&self) -> Option<usize> {
        let sel = self.done_state.selected()?;
        self.filtered_done_indices().get(sel).copied()
    }

    /// Clamp active_state to the current filtered count.
    fn clamp_active_sel(&mut self) {
        let n = self.filtered_active_indices().len();
        match n {
            0 => self.active_state.select(None),
            n => {
                let s = self.active_state.selected().unwrap_or(0).min(n - 1);
                self.active_state.select(Some(s));
            }
        }
    }

    /// Clamp done_state to the current filtered count.
    fn clamp_done_sel(&mut self) {
        let n = self.filtered_done_indices().len();
        match n {
            0 => self.done_state.select(None),
            n => {
                let s = self.done_state.selected().unwrap_or(0).min(n - 1);
                self.done_state.select(Some(s));
            }
        }
    }

    /// Re-clamp both selections after the filter string changed.
    fn on_filter_changed(&mut self) {
        self.clamp_active_sel();
        self.clamp_done_sel();
    }

    pub fn start_search(&mut self) {
        self.input = self.filter.clone();
        self.mode = Mode::Search;
    }

    pub fn search_push(&mut self, c: char) {
        self.input.push(c);
        self.filter = self.input.clone();
        self.on_filter_changed();
    }

    pub fn search_pop(&mut self) {
        self.input.pop();
        self.filter = self.input.clone();
        self.on_filter_changed();
    }

    /// Keep filter, return to Normal.
    pub fn apply_search(&mut self) {
        self.filter = self.input.clone();
        self.input.clear();
        self.mode = Mode::Normal;
    }

    /// Clear filter, return to Normal.
    pub fn cancel_search(&mut self) {
        self.filter.clear();
        self.input.clear();
        self.mode = Mode::Normal;
        self.on_filter_changed();
    }

    /// Clear active filter (Esc in Normal mode).
    pub fn clear_filter(&mut self) {
        if self.filter.is_empty() {
            return;
        }
        // Try to keep the same task selected after widening the view.
        let active_id = self.active_real_idx().map(|i| self.active[i].id);
        let done_id = self.done_real_idx().map(|i| self.done[i].id);
        self.filter.clear();
        if let Some(id) = active_id {
            let idx = self.active.iter().position(|t| t.id == id);
            self.active_state.select(idx);
        }
        if let Some(id) = done_id {
            let idx = self.done.iter().position(|t| t.id == id);
            self.done_state.select(idx);
        }
    }
}

// ─── navigation ──────────────────────────────────────────────────────────────

impl App {
    pub fn switch_focus(&mut self) {
        match self.focus {
            Focus::Active => {
                if self.done_state.selected().is_none() {
                    let n = self.filtered_done_indices().len();
                    if n > 0 {
                        self.done_state.select(Some(0));
                    }
                }
                self.focus = Focus::Done;
            }
            Focus::Done => {
                if self.active_state.selected().is_none() {
                    let n = self.filtered_active_indices().len();
                    if n > 0 {
                        self.active_state.select(Some(0));
                    }
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
                let n = self.filtered_active_indices().len();
                if n == 0 {
                    return;
                }
                let i = self.active_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                self.active_state.select(Some(i));
            }
            Focus::Done => {
                let n = self.filtered_done_indices().len();
                if n == 0 {
                    return;
                }
                let i = self.done_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                self.done_state.select(Some(i));
            }
        }
    }
}

// ─── task actions ────────────────────────────────────────────────────────────

impl App {
    pub fn move_task_up(&mut self) -> Result<()> {
        // Reordering with an active filter is confusing; skip it.
        if self.focus != Focus::Active || !self.filter.is_empty() {
            return Ok(());
        }
        let Some(i) = self.active_state.selected() else { return Ok(()) };
        if i == 0 {
            return Ok(());
        }
        let (pos_a, id_a) = (self.active[i - 1].position, self.active[i - 1].id);
        let (pos_b, id_b) = (self.active[i].position, self.active[i].id);
        self.push_undo(UndoAction::Reorder { id_a, pos_a, id_b, pos_b });
        self.db.set_position(id_a, pos_b)?;
        self.db.set_position(id_b, pos_a)?;
        self.active[i - 1].position = pos_b;
        self.active[i].position = pos_a;
        self.active.swap(i - 1, i);
        self.active_state.select(Some(i - 1));
        Ok(())
    }

    pub fn move_task_down(&mut self) -> Result<()> {
        if self.focus != Focus::Active || !self.filter.is_empty() {
            return Ok(());
        }
        let Some(i) = self.active_state.selected() else { return Ok(()) };
        if i + 1 >= self.active.len() {
            return Ok(());
        }
        let (pos_a, id_a) = (self.active[i].position, self.active[i].id);
        let (pos_b, id_b) = (self.active[i + 1].position, self.active[i + 1].id);
        self.push_undo(UndoAction::Reorder { id_a, pos_a, id_b, pos_b });
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
                let Some(real_i) = self.active_real_idx() else { return Ok(()) };
                let id = self.active[real_i].id;
                self.push_undo(UndoAction::ToggleDone { id, from_done: false });
                self.active[real_i].done = true;
                self.db.set_done(id, true)?;
                let task = self.active.remove(real_i);
                self.clamp_active_sel();
                let prev_sel = self.done_state.selected().map(|s| s + 1).unwrap_or(0);
                self.done.insert(0, task);
                self.done_state.select(Some(prev_sel));
            }
            Focus::Done => {
                let Some(real_i) = self.done_real_idx() else { return Ok(()) };
                let id = self.done[real_i].id;
                self.push_undo(UndoAction::ToggleDone { id, from_done: true });
                let new_pos = self.active.iter().map(|t| t.position).max().unwrap_or(0) + 1;
                self.done[real_i].done = false;
                self.done[real_i].position = new_pos;
                self.db.set_done(id, false)?;
                self.db.set_position(id, new_pos)?;
                let task = self.done.remove(real_i);
                self.clamp_done_sel();
                self.active.push(task);
                if self.active_state.selected().is_none() {
                    let n = self.filtered_active_indices().len();
                    if n > 0 {
                        self.active_state.select(Some(n - 1));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn start_delete(&mut self) {
        let task = match self.focus {
            Focus::Active => self.active_real_idx().and_then(|i| self.active.get(i)),
            Focus::Done => self.done_real_idx().and_then(|i| self.done.get(i)),
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
                let task = self.active.remove(i);
                self.push_undo(UndoAction::DeleteTask { task, was_done: false });
                self.clamp_active_sel();
            } else if let Some(i) = self.done.iter().position(|t| t.id == id) {
                let task = self.done.remove(i);
                self.push_undo(UndoAction::DeleteTask { task, was_done: true });
                self.clamp_done_sel();
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
        self.input.clear();
        self.mode = Mode::Input;
    }

    pub fn cancel_input(&mut self) {
        self.input.clear();
        self.mode = Mode::Normal;
    }

    pub fn confirm_input(&mut self) -> Result<()> {
        let title = self.input.trim().to_string();
        self.input.clear();
        self.mode = Mode::Normal;
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
            tags: Vec::new(),
        });
        // Select the new task even if it doesn't match the filter.
        self.filter.clear();
        self.on_filter_changed();
        self.active_state.select(Some(self.active.len() - 1));
        Ok(())
    }
}

// ─── rename ──────────────────────────────────────────────────────────────────

impl App {
    pub fn start_edit_title(&mut self) {
        let title = match self.focus {
            Focus::Active => self.active_real_idx().and_then(|i| self.active.get(i)).map(|t| t.title.clone()),
            Focus::Done => self.done_real_idx().and_then(|i| self.done.get(i)).map(|t| t.title.clone()),
        };
        if let Some(title) = title {
            self.input = title;
            self.mode = Mode::EditTitle;
        }
    }

    pub fn confirm_edit_title(&mut self) -> Result<()> {
        let new_title = self.input.trim().to_string();
        self.input.clear();
        self.mode = Mode::Normal;
        if new_title.is_empty() {
            return Ok(());
        }
        let (id, old_title) = match self.focus {
            Focus::Active => {
                let Some(i) = self.active_real_idx() else { return Ok(()) };
                let id = self.active[i].id;
                let old = std::mem::replace(&mut self.active[i].title, new_title.clone());
                (id, old)
            }
            Focus::Done => {
                let Some(i) = self.done_real_idx() else { return Ok(()) };
                let id = self.done[i].id;
                let old = std::mem::replace(&mut self.done[i].title, new_title.clone());
                (id, old)
            }
        };
        self.push_undo(UndoAction::EditTitle { id, old_title });
        self.db.update_title(id, &new_title)?;
        Ok(())
    }

    pub fn cancel_edit_title(&mut self) {
        self.input.clear();
        self.mode = Mode::Normal;
    }
}

// ─── tags ────────────────────────────────────────────────────────────────────

impl App {
    pub fn start_edit_tags(&mut self) {
        let tags_str = match self.focus {
            Focus::Active => self.active_real_idx().and_then(|i| self.active.get(i)).map(|t| t.tags.join(", ")),
            Focus::Done => self.done_real_idx().and_then(|i| self.done.get(i)).map(|t| t.tags.join(", ")),
        };
        if let Some(s) = tags_str {
            self.input = s;
            self.mode = Mode::EditTags;
        }
    }

    pub fn confirm_edit_tags(&mut self) -> Result<()> {
        let raw = self.input.clone();
        self.input.clear();
        self.mode = Mode::Normal;
        let new_tags: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let (id, old_tags) = match self.focus {
            Focus::Active => {
                let Some(i) = self.active_real_idx() else { return Ok(()) };
                let id = self.active[i].id;
                let old = std::mem::replace(&mut self.active[i].tags, new_tags.clone());
                (id, old)
            }
            Focus::Done => {
                let Some(i) = self.done_real_idx() else { return Ok(()) };
                let id = self.done[i].id;
                let old = std::mem::replace(&mut self.done[i].tags, new_tags.clone());
                (id, old)
            }
        };
        self.push_undo(UndoAction::EditTags { id, old_tags });
        self.db.set_tags(id, &new_tags.join(","))?;
        Ok(())
    }

    pub fn cancel_edit_tags(&mut self) {
        self.input.clear();
        self.mode = Mode::Normal;
    }
}

// ─── notes ───────────────────────────────────────────────────────────────────

impl App {
    pub fn start_edit_note(&mut self) {
        let task = match self.focus {
            Focus::Active => self.active_real_idx().and_then(|i| self.active.get(i)),
            Focus::Done => self.done_real_idx().and_then(|i| self.done.get(i)),
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
        editor.move_cursor(tui_textarea::CursorMove::Bottom);
        editor.move_cursor(tui_textarea::CursorMove::End);
        self.note_editor = Some(editor);
        self.mode = Mode::EditNote;
    }

    pub fn confirm_note(&mut self) -> Result<()> {
        if let Some(editor) = self.note_editor.take() {
            let new_notes = editor.lines().join("\n");
            let (id, old_notes) = match self.focus {
                Focus::Active => {
                    let Some(i) = self.active_real_idx() else {
                        self.mode = Mode::Normal;
                        return Ok(());
                    };
                    let id = self.active[i].id;
                    let old = std::mem::replace(&mut self.active[i].notes, new_notes.clone());
                    (id, old)
                }
                Focus::Done => {
                    let Some(i) = self.done_real_idx() else {
                        self.mode = Mode::Normal;
                        return Ok(());
                    };
                    let id = self.done[i].id;
                    let old = std::mem::replace(&mut self.done[i].notes, new_notes.clone());
                    (id, old)
                }
            };
            self.push_undo(UndoAction::EditNotes { id, old_notes });
            self.db.set_notes(id, &new_notes)?;
        }
        self.mode = Mode::Normal;
        Ok(())
    }

    pub fn cancel_note(&mut self) {
        self.note_editor = None;
        self.mode = Mode::Normal;
    }

    pub fn selected_notes(&self) -> &str {
        match self.focus {
            Focus::Active => self
                .active_real_idx()
                .and_then(|i| self.active.get(i))
                .map(|t| t.notes.as_str())
                .unwrap_or(""),
            Focus::Done => self
                .done_real_idx()
                .and_then(|i| self.done.get(i))
                .map(|t| t.notes.as_str())
                .unwrap_or(""),
        }
    }
}

// ─── undo ────────────────────────────────────────────────────────────────────

impl App {
    fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push_back(action);
        if self.undo_stack.len() > UNDO_LIMIT {
            self.undo_stack.pop_front();
        }
    }

    pub fn undo(&mut self) -> Result<()> {
        let Some(action) = self.undo_stack.pop_back() else { return Ok(()) };
        match action {
            UndoAction::DeleteTask { task, was_done } => {
                self.db.restore_task(&task)?;
                if was_done {
                    self.done.insert(0, task);
                    if self.done_state.selected().is_none() {
                        self.done_state.select(Some(0));
                    }
                } else {
                    self.active.push(task);
                    self.active.sort_by_key(|t| t.position);
                    self.clamp_active_sel();
                    if self.active_state.selected().is_none() {
                        self.active_state.select(Some(0));
                    }
                }
            }

            UndoAction::ToggleDone { id, from_done } => {
                if from_done {
                    // Was done → moved to active. Undo: move back to done.
                    if let Some(i) = self.active.iter().position(|t| t.id == id) {
                        self.active[i].done = true;
                        self.db.set_done(id, true)?;
                        let task = self.active.remove(i);
                        self.clamp_active_sel();
                        let prev = self.done_state.selected().map(|s| s + 1).unwrap_or(0);
                        self.done.insert(0, task);
                        self.done_state.select(Some(prev));
                    }
                } else {
                    // Was active → moved to done. Undo: move back to active.
                    if let Some(i) = self.done.iter().position(|t| t.id == id) {
                        self.done[i].done = false;
                        self.db.set_done(id, false)?;
                        let task = self.done.remove(i);
                        self.clamp_done_sel();
                        self.active.push(task);
                        self.active.sort_by_key(|t| t.position);
                        self.clamp_active_sel();
                    }
                }
            }

            UndoAction::EditTitle { id, old_title } => {
                self.db.update_title(id, &old_title)?;
                if let Some(t) = self.active.iter_mut().find(|t| t.id == id) {
                    t.title = old_title;
                } else if let Some(t) = self.done.iter_mut().find(|t| t.id == id) {
                    t.title = old_title;
                }
            }

            UndoAction::EditTags { id, old_tags } => {
                self.db.set_tags(id, &old_tags.join(","))?;
                if let Some(t) = self.active.iter_mut().find(|t| t.id == id) {
                    t.tags = old_tags;
                } else if let Some(t) = self.done.iter_mut().find(|t| t.id == id) {
                    t.tags = old_tags;
                }
            }

            UndoAction::EditNotes { id, old_notes } => {
                self.db.set_notes(id, &old_notes)?;
                if let Some(t) = self.active.iter_mut().find(|t| t.id == id) {
                    t.notes = old_notes;
                } else if let Some(t) = self.done.iter_mut().find(|t| t.id == id) {
                    t.notes = old_notes;
                }
            }

            UndoAction::Reorder { id_a, pos_a, id_b, pos_b } => {
                self.db.set_position(id_a, pos_a)?;
                self.db.set_position(id_b, pos_b)?;
                for t in self.active.iter_mut() {
                    if t.id == id_a { t.position = pos_a; }
                    else if t.id == id_b { t.position = pos_b; }
                }
                self.active.sort_by_key(|t| t.position);
                self.clamp_active_sel();
            }
        }
        Ok(())
    }
}
