# Hollow

Distraction-free terminal writing environment. Just you and the words.

## Why Hollow?

Writing is hard. Distraction makes it impossible.

Modern writing tools fight against focus with formatting options, cloud sync indicators, collaboration features, and notifications. Every feature adds friction between thought and page.

The terminal is the last sanctuary of focus. No notifications. No mouse temptation. No visual clutter. Just text and a blinking cursor.

Hollow is a writing environment that embraces the terminal's focus while understanding that writing isn't just editing text - it's thinking made visible.

## Features

- Full-screen, minimal interface - no chrome, no distractions
- Modal editing (Write and Navigate modes)
- Auto-save every 30 seconds
- Word count and session time tracking
- Search with case-insensitive matching
- Undo/redo
- Configurable text width with centered layout
- Vim-style navigation in Navigate mode

## Installation

### From Source

```bash
git clone https://github.com/katieblackabee/hollow.git
cd hollow
cargo install --path .
```

### From crates.io (coming soon)

```bash
cargo install hollow
```

## Usage

```bash
hollow <file>
```

Opens the file for editing. Creates it if it doesn't exist.

### Options

```
--help, -h          Show help message
--version, -v       Show version
--width <N>         Set text width (default: 80)
--no-autosave       Disable auto-save
```

## Key Bindings

### Universal (work in all modes)

| Key | Action |
|-----|--------|
| Ctrl+S | Save file |
| Ctrl+Q | Quit (prompts if unsaved) |
| Ctrl+G | Toggle status line |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |

### Write Mode

Type normally. All printable characters are inserted at the cursor.

| Key | Action |
|-----|--------|
| Escape | Enter Navigate mode |
| Enter | Insert newline |
| Backspace | Delete character before cursor |
| Delete | Delete character at cursor |
| Arrow keys | Move cursor |
| Ctrl+Left/Right | Move by word |
| Home/End | Line start/end |
| Ctrl+Home/End | Document start/end |
| Page Up/Down | Move by page |

### Navigate Mode

Press `Escape` from Write mode to enter Navigate mode.

| Key | Action |
|-----|--------|
| i | Return to Write mode |
| h/j/k/l | Move left/down/up/right |
| w/b | Move forward/backward by word |
| 0/$ | Line start/end |
| gg | Document start |
| G | Document end |
| / | Start search |
| n/N | Next/previous search match |
| dd | Delete current line |
| yy | Copy current line |
| p | Paste |
| u | Undo |
| Ctrl+r | Redo |
| ? | Show help |

### Search Mode

Press `/` in Navigate mode to start a search.

| Key | Action |
|-----|--------|
| Enter | Execute search |
| Escape | Cancel search |
| Backspace | Delete character |

## Configuration

Hollow looks for a config file at `~/.config/hollow/config.toml`.

```toml
[editor]
text_width = 80           # Characters per line before wrapping
tab_width = 4             # Spaces per tab
auto_save_seconds = 30    # Auto-save interval (0 to disable)

[display]
show_status = false       # Show status line by default
status_timeout = 3        # Seconds before status auto-hides (0 = never)
```

All settings have sensible defaults - configuration is entirely optional.

## Philosophy

Hollow is built on a few core beliefs:

1. **Less is more.** Every feature should earn its place by helping you write.

2. **The terminal is a feature, not a limitation.** It provides focus that GUI apps struggle to match.

3. **Your words belong to you.** Files are plain Markdown, stored locally. No cloud, no lock-in.

4. **Writing is a practice.** Session tracking helps build consistent habits.

## Roadmap

### v0.1 (Current)
- [x] Core editing
- [x] Modal navigation
- [x] Auto-save
- [x] Word count
- [x] Search
- [x] Configuration

### v0.2 (Planned)
- [ ] Search highlighting
- [ ] Improved word wrapping
- [ ] Daily goals and streaks
- [ ] Version history

### Future
- [ ] Multiple documents (projects)
- [ ] Export to HTML/PDF
- [ ] Spell checking integration
- [ ] Custom themes

## Building from Source

Requirements:
- Rust 1.70 or later
- Cargo

```bash
# Clone the repository
git clone https://github.com/katieblackabee/hollow.git
cd hollow

# Build release version
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Author

Katie the Clawdius Prime <blackabee@gmail.com>

---

*In the quiet of the terminal, your words find their way.*
