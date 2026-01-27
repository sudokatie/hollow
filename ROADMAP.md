# Hollow Roadmap

Next steps for future development sessions.

---

## v0.2 - Habits and History

### Daily Goals and Streaks
- Configurable daily word count goal
- Track consecutive days of meeting goal
- Subtle indicator in status line (e.g., "Day 5" or flame icon)
- Store streak data in ~/.config/hollow/stats.json

### Version History
- Save timestamped snapshots on each save
- Store in ~/.local/share/hollow/history/<filename>/
- Command to list versions: `hollow --history <file>`
- Command to restore: `hollow --restore <file> <timestamp>`
- Configurable max versions to keep

### Writing Statistics
- Track words written per session
- Track total time spent writing
- Track words per minute (when actively typing)
- Weekly/monthly summaries
- Command: `hollow --stats`

---

## Future

### Multiple Documents (Projects)
- Project mode: `hollow --project <dir>`
- Sidebar file list (toggleable)
- Quick switch between files
- Project-level word count goals

### Export to HTML/PDF
- `hollow --export html <file>`
- `hollow --export pdf <file>`
- Respect markdown formatting
- Configurable CSS for HTML export
- Use system tools for PDF (pandoc or similar)

### Spell Checking Integration
- Optional integration with aspell/hunspell
- Highlight misspelled words (subtle underline)
- Navigate to next misspelling
- Add to dictionary
- Toggle on/off

### Custom Themes
- Theme files in ~/.config/hollow/themes/
- Configure colors: text, background, status, cursor, highlights
- Ship with 2-3 built-in themes
- `hollow --theme <name>`

---

## Notes

- Keep the distraction-free philosophy - every feature must earn its place
- No feature should require the mouse
- All data stays local
- Plain text files always readable without Hollow
