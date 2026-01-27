# Hollow

Distraction-free terminal writing environment. Just you and the words.

## Features

- Full-screen, minimal interface
- Modal editing (Write and Navigate modes)
- Auto-save every 30 seconds
- Word count and session tracking
- Search with highlighting
- Undo/redo with intelligent grouping
- Soft word wrapping
- Configurable text width

## Installation

```bash
cargo install --path .
```

## Usage

```bash
hollow <file>
```

Opens the file for editing. Creates it if it doesn't exist.

## Key Bindings

### Universal (both modes)

| Key | Action |
|-----|--------|
| Ctrl+S | Save |
| Ctrl+Q | Quit |
| Ctrl+G | Toggle status line |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Escape | Enter Navigate mode |

### Write Mode

Type normally. All printable characters are inserted at the cursor.

### Navigate Mode

| Key | Action |
|-----|--------|
| h/j/k/l | Move left/down/up/right |
| w/b | Move by word |
| 0/$ | Line start/end |
| gg/G | Document start/end |
| / | Search |
| n/N | Next/previous match |
| dd | Delete line |
| yy | Copy line |
| p | Paste |
| u | Undo |
| Ctrl+r | Redo |
| i | Return to Write mode |
| ? | Show help |

## Configuration

Config file: `~/.config/hollow/config.toml`

```toml
[editor]
text_width = 80
tab_width = 4
auto_save_seconds = 30

[display]
show_status = false
status_timeout = 3
```

## License

MIT
