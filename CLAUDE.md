# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build              # compile (dev)
cargo build --release    # compile optimised binary → target/release/prioritize.exe
cargo run                # build and run
cargo clippy             # lint (zero warnings expected)
cargo fmt                # format
cargo check              # fast type-check without linking
```

The binary is named `prioritize` (see `[[bin]]` in Cargo.toml).

## Architecture

Five source files with strict separation of concerns:

| File | Role |
|------|------|
| `src/main.rs` | Terminal setup/teardown, raw event loop |
| `src/app.rs` | All mutable application state and business logic |
| `src/db.rs` | SQLite CRUD via rusqlite — no business logic |
| `src/task.rs` | Plain `Task` data struct |
| `src/ui.rs` | Stateless rendering functions (ratatui widgets) |

**State flow:** `main` reads crossterm events → calls methods on `App` → `App` updates its vecs + calls `Database` to persist → `main` calls `ui::render(frame, app)` to redraw.

**Two-column layout:** tasks are stored in two separate `Vec<Task>` fields:
- `App::active` — incomplete tasks, ordered by `position` (user-reorderable with K/J)
- `App::done` — completed tasks, ordered by `id DESC` (most recently completed first)

Each column has its own `ListState`. `App::focus` (`Focus::Active` / `Focus::Done`) tracks which column the user is in; Tab switches between them.

**Priority ordering:** active tasks have an integer `position` field. Reordering swaps the `position` values of adjacent tasks in both the in-memory vec and SQLite simultaneously.

**Modes (`app::Mode`):** `Normal`, `Input` (typing a new task), `Confirm` (delete confirmation). All three are `Copy` enums to avoid borrow issues in `match`.

**Delete flow:** `start_delete()` saves `(id, title)` in `App::pending_delete` and sets `Mode::Confirm`. A centered popup renders over the UI. `confirm_delete()` / `cancel_confirm()` resolve it.

**Date tracking:** `created_at` is stored as `TEXT "YYYY-MM-DD"` in SQLite. Active tasks are coloured by age: yellow ≥7 days, red ≥14 days (`ui::staleness_style`).

**Database location:** `dirs::data_dir()/prioritize/tasks.db`
- Windows: `%APPDATA%\prioritize\tasks.db`
- Falls back to `./tasks.db` if the data dir is unavailable.

**DB migration:** the `open()` function runs `ALTER TABLE tasks ADD COLUMN created_at ...` after `CREATE TABLE IF NOT EXISTS`, so existing databases gain the column automatically (the error is silently ignored when the column already exists).

## Linux build requirements

The `bundled` feature in rusqlite compiles SQLite from source, so a C compiler (`gcc` or `clang`) must be available. All other crates are pure Rust or use crossterm's cross-platform terminal abstractions. No other system packages are required.

Database path on Linux: `$HOME/.local/share/prioritize/tasks.db` (via XDG spec).

## Key bindings (Normal mode)

| Key | Action |
|-----|--------|
| `n` | New task (Input mode) |
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `J` (Shift+J) | Move selected task down in priority (Active column only) |
| `K` (Shift+K) | Move selected task up in priority (Active column only) |
| `Space` | Toggle done — moves task between columns |
| `e` | Open note editor for selected task |
| `d` | Delete with confirmation popup |
| `Tab` | Switch focus between Active and Completed columns |
| `q` / Ctrl+C | Quit |

In **note editor** (`Mode::EditNote`): full multiline editing via `tui-textarea`; `Ctrl+S` saves, `Esc` discards. A `·` marker in the task list indicates a task has notes. The notes panel (always visible at the bottom) shows the notes of the currently selected task.
