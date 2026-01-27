# Changelog

All notable changes to Hollow will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-01-27

### Added
- Paragraph navigation with { and } keys
- line_spacing config option

### Fixed
- Status line format now matches spec exactly

## [0.1.1] - 2026-01-27

### Added
- Search navigation with n/N (next/previous match)
- Backup file creation on first edit (.hollow-backup)
- Minimum terminal size check (40x10)
- Search match highlighting

### Fixed
- Undo grouping now correctly groups rapid edits within 2 seconds
- Search properly clears when cancelled

## [0.1.0] - 2026-01-27

### Added
- Initial release
- Core text editing with rope data structure
- Modal editing (Write and Navigate modes)
- Vim-style navigation (h/j/k/l, w/b, gg/G)
- Auto-save with configurable interval
- Word count and session time tracking
- Case-insensitive search
- Undo/redo functionality
- Configurable text width
- Status line with toggle
- Help overlay
- Quit confirmation for unsaved changes
- Configuration file support (~/.config/hollow/config.toml)
- CLI options: --width, --no-autosave, --help, --version
