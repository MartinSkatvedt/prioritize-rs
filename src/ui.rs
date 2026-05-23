use chrono::{Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, Focus, Mode};
use crate::task::Task;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    render_active_list(frame, app, cols[0]);
    render_done_list(frame, app, cols[1]);
    render_notes_panel(frame, app, rows[1]);
    render_status_bar(frame, app, rows[2]);

    match app.mode {
        Mode::Input => render_input_popup(frame, app, area),
        Mode::Confirm => render_confirm_popup(frame, app, area),
        Mode::EditNote => render_note_editor(frame, app, area),
        Mode::Normal => {}
    }
}

fn task_item(task: &Task, index: usize, is_done: bool) -> ListItem<'static> {
    let note_mark = if task.notes.is_empty() { " " } else { "·" };
    let date = short_date(&task.created_at);
    let label = format!(
        "  {:>2}.  {}  {}  {}",
        index + 1,
        date,
        note_mark,
        task.title
    );
    let style = if is_done {
        Style::default().fg(Color::DarkGray)
    } else {
        staleness_style(&task.created_at)
    };
    ListItem::new(Line::from(Span::styled(label, style)))
}

fn column_styles(focused: bool) -> (Style, Style) {
    let border = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default()
    };
    let highlight = if focused {
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray)
    };
    (border, highlight)
}

fn render_active_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Active;
    let (border_style, highlight_style) = column_styles(focused);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Active ")
        .border_style(border_style);

    if app.active.is_empty() {
        frame.render_widget(
            Paragraph::new("\n  No active tasks — press 'n' to add one.")
                .block(block)
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .active
        .iter()
        .enumerate()
        .map(|(i, t)| task_item(t, i, false))
        .collect();

    frame.render_stateful_widget(
        List::new(items).block(block).highlight_style(highlight_style),
        area,
        &mut app.active_state,
    );
}

fn render_done_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Done;
    let (border_style, highlight_style) = column_styles(focused);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Completed ")
        .border_style(border_style);

    if app.done.is_empty() {
        frame.render_widget(
            Paragraph::new("\n  No completed tasks yet.")
                .block(block)
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .done
        .iter()
        .enumerate()
        .map(|(i, t)| task_item(t, i, true))
        .collect();

    frame.render_stateful_widget(
        List::new(items).block(block).highlight_style(highlight_style),
        area,
        &mut app.done_state,
    );
}

fn render_notes_panel(frame: &mut Frame, app: &App, area: Rect) {
    let notes = app.selected_notes();
    let (content, style) = if notes.is_empty() {
        (
            "  No notes — press 'e' to add.",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (notes, Style::default())
    };
    frame.render_widget(
        Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title(" Notes "))
            .style(style),
        area,
    );
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help = match app.mode {
        Mode::Normal => " n:new  j/k:navigate  K/J:reorder  tab:switch  space:toggle  e:edit notes  d:delete  q:quit",
        Mode::Input => " Enter:confirm  Esc:cancel",
        Mode::Confirm => " y/Enter:confirm delete  n/Esc:cancel",
        Mode::EditNote => " Ctrl+S:save  Esc:discard",
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn render_input_popup(frame: &mut Frame, app: &App, area: Rect) {
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_width, 3, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Paragraph::new(format!(" > {}_", app.input))
            .block(Block::default().borders(Borders::ALL).title(" New Task ")),
        popup_area,
    );
}

fn render_confirm_popup(frame: &mut Frame, app: &App, area: Rect) {
    let title = app
        .pending_delete
        .as_ref()
        .map(|(_, t)| t.as_str())
        .unwrap_or("");
    let popup_width = 54.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_width, 5, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Paragraph::new(format!("\n  Delete \"{}\"?\n\n      [y] Yes     [n] No", title))
            .block(Block::default().borders(Borders::ALL).title(" Delete Task ")),
        popup_area,
    );
}

fn render_note_editor(frame: &mut Frame, app: &App, area: Rect) {
    let Some(editor) = app.note_editor.as_ref() else {
        return;
    };
    let popup_width = area.width.saturating_sub(6).max(20);
    let popup_height = (area.height.saturating_sub(4)).clamp(5, 24);
    let popup_area = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(editor, popup_area);
}

fn short_date(s: &str) -> String {
    if s.len() >= 10 {
        s[5..10].to_string()
    } else {
        "     ".to_string()
    }
}

fn staleness_style(created_at: &str) -> Style {
    if created_at.is_empty() {
        return Style::default();
    }
    if let Ok(date) = NaiveDate::parse_from_str(created_at, "%Y-%m-%d") {
        let days = (Local::now().date_naive() - date).num_days();
        if days >= 14 {
            return Style::default().fg(Color::Red);
        } else if days >= 7 {
            return Style::default().fg(Color::Yellow);
        }
    }
    Style::default()
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(r.width), height.min(r.height))
}
