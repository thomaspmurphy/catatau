# Catatau - Terminal EPUB Reader

<img width="1036" height="296" alt="Screenshot 2025-08-27 at 08 31 35" src="https://github.com/user-attachments/assets/4b2f974b-a9fc-492b-a6b1-31b39f615e85" />

A fast, lightweight terminal-based EPUB reader built in Rust.

## Current Status ✅

- Parse EPUB files and extract chapters
- Terminal UI with book metadata display
- Chapter navigation (←/→ or h/l)
- Text scrolling (↑/↓ or j/k)
- Page navigation (Space/b)
- Quick jumps (g/G for start/end)

## Installation

### Install from Source
```bash
cargo install --git https://github.com/thomaspmurphy/catatau
```

### Install from Local Directory
```bash
git clone https://github.com/thomaspmurphy/catatau
cd catatau
cargo install --path .
```

## Usage

```bash
ctt path/to/book.epub
```

The application will be installed as `ctt` and available globally in your PATH.

Keyboard controls are loosely inspired by vim.

**Keyboard Controls:**

- `q` - quit
- `↑↓` or `jk` - scroll line by line
- `←→` or `hl` - previous/next chapter
- `Space`/`b` - page down/up
- `g`/`G` - beginning/end of chapter
- `/` - fuzzy find in book
- `-` - open contents for quick jump

## To Do (Maintenance)

- [ ] Refactor the UI module (separate rendering and event handling and better
      organisation)
- [ ] Improve lazy loading for performance with large books

## Development Roadmap

### Core Features

- [ ] **MOBI Support**
- [ ] **Library Management** - Browse and organise multiple books
- [ ] **Bookmarking System** - Save and restore reading positions
- [ ] **Full-text search** - Currently we have fuzzy finding which seems extremely effective, but FTS would be great
- [ ] **Annotations & Highlights** - Mark important passages with notes

### Text Rendering & UI

- [ ] **Enhanced Formatting** - Better HTML/CSS parsing for rich text (bold, italic, headers)
- [ ] **Text Reflow** - Intelligent word wrapping that preserves formatting
- [ ] **Pagination Improvements** - Natural page breaks and chapter transitions
- [ ] **Progress Indicators** - Reading progress, chapter info, time estimates
- [ ] Create a GUI version using [dioxsus](https://dioxuslabs.com/)

### Advanced Features

- [ ] **Multi-format Support** - Abstract parser layer for EPUB/MOBI
- [ ] **Vim-like Navigation** - Advanced keybindings for power users
  - "v" mode for highlighting
  - "i" mode for annotations (changing mode at position will open up an inline text box to store the annotaiton)
- [ ] **Modal Interface** - Library browser, search overlay, bookmark manager
- [ ] **Unicode Support** - Proper handling of all forms of international text
- [ ] **Export Functionality** - Export annotations and highlights

## Technical Architecture

### Current Stack

- **UI**: `ratatui` + `crossterm` for terminal interface
- **Parsing**: `zip` + `quick-xml` for EPUB extraction
- **Text Processing**: `html2text` for content conversion
- **CLI**: `clap` for command-line arguments

### Planned Dependencies

- `rusqlite` - Database for library and annotations
- `tantivy` - Full-text search and indexing
- `serde` - Configuration serialisation
- `tokio` - Async file operations

### Architecture Layers

1. **File Format Layer** - EPUB/MOBI parsers with common interface
2. **Content Model** - Unified book structure representation
3. **Rendering Pipeline** - HTML → styling → terminal output
4. **UI Controller** - Event handling and state management
5. **Storage Layer** - Database operations and indexing

## Contributing

This is an early-stage project. The current focus is on building robust core functionality before adding advanced features. The most challenging aspects will be:

- Implementing proper HTML/CSS rendering for terminal output
- Creating smooth, natural text flow and pagination
- Building fast search indexing for large libraries
- Designing an intuitive modal interface system

## Licence

MIT
