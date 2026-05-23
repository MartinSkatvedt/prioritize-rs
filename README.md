# prioritize

A terminal task manager with priority ordering, notes, and staleness tracking.

```
┌─ Active ──────────────────────────┬─ Completed ───────────────────────┐
│   1.  05-20  ·  Write tests       │   1.  05-21  ·  Set up CI         │
│   2.  05-21     Fix login bug     │   2.  05-22     Update docs       │
│   3.  05-10     Update deps       │                                   │
└───────────────────────────────────┴───────────────────────────────────┘
┌─ Notes ───────────────────────────────────────────────────────────────┐
│ Cover auth module and all API endpoints.                              │
│                                                                       │
└───────────────────────────────────────────────────────────────────────┘
 n:new  j/k:navigate  K/J:reorder  tab:switch  space:toggle  e:edit notes  d:delete  q:quit
```

Tasks older than **7 days** are highlighted yellow; older than **14 days** turn red.

## Features

- Create and delete tasks (delete requires confirmation)
- Drag tasks up/down to set priority order
- Toggle tasks between **Active** and **Completed** columns
- Attach multiline notes to any task and edit them in-app
- Date-added tracking with staleness colour coding
- State persisted in a local SQLite database

## Installation

### Linux

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/MartinSkatvedt/prioritize-rs/releases/latest/download/prioritize-installer.sh | sh
```

Supports `x86_64` and `aarch64`. The installer also places a `prioritize-update` binary on your `PATH` — run it any time to upgrade to the latest release.

### Windows

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/MartinSkatvedt/prioritize-rs/releases/latest/download/prioritize-installer.ps1 | iex"
```

### Pre-built binaries

Download the archive for your platform from the [Releases](https://github.com/MartinSkatvedt/prioritize-rs/releases) page and place the `prioritize` binary somewhere on your `PATH`.

### Build from source

Requires a Rust toolchain and a C compiler (`gcc` or `clang`) for the bundled SQLite.

```sh
git clone https://github.com/MartinSkatvedt/prioritize-rs
cd prioritize-rs
cargo build --release
```

The binary is at `target/release/prioritize` (`prioritize.exe` on Windows).

## Usage

```sh
prioritize
```

### Key bindings

| Key | Action |
|-----|--------|
| `n` | New task |
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `K` | Move task up in priority |
| `J` | Move task down in priority |
| `Space` | Toggle done / undone |
| `e` | Edit notes for selected task |
| `d` | Delete selected task (asks for confirmation) |
| `Tab` | Switch focus between Active and Completed |
| `q` / `Ctrl+C` | Quit |

**Note editor:** `Ctrl+S` to save, `Esc` to discard. A `·` in the task list marks tasks that have notes.

## Data

Tasks are stored in a SQLite database at:

| Platform | Path |
|----------|------|
| Windows | `%APPDATA%\prioritize\tasks.db` |
| Linux | `~/.local/share/prioritize/tasks.db` |
| macOS | `~/Library/Application Support/prioritize/tasks.db` |
