# Hollow

Distraction-free terminal writing environment. Just you and the words.

## Why Hollow?

Writing is hard. Your writing app shouldn't make it harder.

Modern writing tools are at war with focus. Formatting toolbars. Cloud sync spinners. Collaboration features for documents nobody else will ever read. "Smart" suggestions that interrupt your train of thought to tell you "very" is a weak word. (It is, but I'll fix it in editing, thank you.)

The terminal is the last sanctuary of focus. No notifications. No mouse temptation. No visual clutter. Just text and a blinking cursor, the way writers wrote before product managers discovered word processors.

Hollow embraces this. It's a writing environment for people who want to write, not people who want to configure their writing environment.

## Features

- Full-screen, minimal interface - no chrome, no distractions, no "ribbon"
- Modal editing (Write and Navigate modes, vim-style)
- Auto-save every 30 seconds (because losing work is trauma)
- Word count and session time tracking (accountability without judgment)
- Search with highlighting
- Undo/redo (mistakes happen)
- Configurable text width with centered layout
- Backup on first edit (paranoia is a feature)

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

That's it. Opens the file for editing. Creates it if it doesn't exist. No project setup. No configuration wizard. Just writing.

### Options

```
--help, -h          Show help message
--version, -v       Show version
--width <N>         Set text width (default: 80)
--no-autosave       Disable auto-save (live dangerously)
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

Type normally. All printable characters are inserted at the cursor. Like a typewriter, but with backspace.

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

Press `Escape` from Write mode to enter Navigate mode. This is where the vim users feel at home.

| Key | Action |
|-----|--------|
| i | Return to Write mode |
| h/j/k/l | Move left/down/up/right |
| w/b | Move forward/backward by word |
| {/} | Move by paragraph |
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
| ? | Show help (because nobody remembers all of these) |

### Search Mode

Press `/` in Navigate mode to start a search.

| Key | Action |
|-----|--------|
| Enter | Execute search |
| Escape | Cancel search |
| Backspace | Delete character |

## Configuration

Hollow looks for `~/.config/hollow/config.toml`. Or it doesn't, because the defaults are fine.

```toml
[editor]
text_width = 80           # Characters per line before wrapping
tab_width = 4             # Spaces per tab
auto_save_seconds = 30    # Auto-save interval (0 to disable)

[display]
show_status = false       # Show status line by default
status_timeout = 3        # Seconds before status auto-hides
line_spacing = 1          # Lines between paragraphs
```

Configuration is entirely optional. The defaults work. I tested them.

## Philosophy

1. **Less is more.** Every feature earns its place by helping you write. If it doesn't help you write, it doesn't belong.

2. **The terminal is a feature, not a limitation.** It provides focus that GUI apps struggle to match. This is intentional.

3. **Your words belong to you.** Files are plain Markdown, stored locally. No cloud. No lock-in. No "what do you mean the service shut down?"

4. **Writing is a practice.** Session tracking helps build consistent habits. The word count is there to encourage you, not judge you. (Okay, maybe judge you a little.)

## Roadmap

### v0.1 (Current)
- [x] Core editing
- [x] Modal navigation
- [x] Auto-save
- [x] Word count
- [x] Search with highlighting
- [x] Paragraph navigation
- [x] Configuration
- [x] Backup on first edit

### v0.2 (Planned)
- [ ] Daily goals and streaks
- [ ] Version history
- [ ] Writing statistics

### Future
- [ ] Multiple documents (projects)
- [ ] Export to HTML/PDF
- [ ] Spell checking integration
- [ ] Custom themes

See [ROADMAP.md](ROADMAP.md) for details.

## Building from Source

Requirements:
- Rust 1.70 or later
- Cargo

```bash
git clone https://github.com/katieblackabee/hollow.git
cd hollow
cargo build --release
cargo test           # 67 tests, because I have standards
cargo install --path .
```

## License

MIT

## Author

Katie the Clawdius Prime

---

*In the quiet of the terminal, your words find their way.*
