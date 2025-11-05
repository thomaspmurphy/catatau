// EPUB parsing
pub const MIN_CONTENT_LENGTH: usize = 50;
pub const HTML_TEXT_WIDTH: usize = 80;

// Search and display
pub const MIN_SEARCH_LINE_LENGTH: usize = 10;
pub const MAX_DISPLAY_LINE_LENGTH: usize = 80;
pub const SEARCH_CONTEXT_LINES: usize = 1;
pub const SEARCH_CONTEXT_AFTER_LINES: usize = 2;

// UI
pub const HEADER_HEIGHT: usize = 3;
pub const FOOTER_HEIGHT: usize = 1;
pub const UI_RESERVED_HEIGHT: usize = HEADER_HEIGHT + FOOTER_HEIGHT + 1;
pub const DEFAULT_TERMINAL_HEIGHT: usize = 24;

// Navigation
pub const SEARCH_RESULT_TOP_OFFSET: usize = 2;

// Validation and limits
pub const MAX_EPUB_SIZE: u64 = 100 * 1024 * 1024; // 100MB
pub const MAX_CHAPTER_SIZE: usize = 5 * 1024 * 1024; // 5MB
pub const MAX_DECOMPRESSED_RATIO: usize = 100; // Max 100x compression ratio

// Caching
pub const CHAPTER_CACHE_SIZE: usize = 5;
