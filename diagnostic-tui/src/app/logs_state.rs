//! State for the Logs tab.

use super::{
    filters::{LevelFilter, LogFileFilter, SourceFilter},
    navigation::ensure_cursor_visible,
};
use diagnostic_parser::LogEntry;
use ratatui::widgets::{ScrollbarState, TableState};

/// Persistent state for the Logs tab.
pub struct LogsState<'a> {
    // -- List pane --
    /// Selected row in the log list.
    pub list_state: TableState,
    /// Scrollbar state for the log list.
    pub list_scrollbar: ScrollbarState,
    /// Anchor index for visual selection on the list.
    pub select_anchor: Option<usize>,

    // -- Detail pane --
    /// Vertical scroll offset inside the log detail pane.
    pub detail_scroll: u16,
    /// Cursor line index in the log detail content.
    pub detail_cursor: usize,
    /// Anchor line for visual selection on the detail pane.
    pub detail_select_anchor: Option<usize>,
    /// Total lines in the log detail content (set during rendering).
    pub detail_line_count: usize,
    /// Whether the detail pane is in select mode.
    pub detail_selecting: bool,
    /// Whether the log detail pane is visible.
    pub show_detail: bool,
    /// Whether the detail pane is focused.
    pub detail_focused: bool,

    // -- Filtering --
    /// The search query string.
    pub search_query: String,
    /// Number of search hits.
    pub search_hits: usize,
    /// Log level filter.
    pub level_filter: LevelFilter,
    /// Source component filter.
    pub source_filter: SourceFilter<'a>,
    /// Log file filter.
    pub log_file_filter: LogFileFilter<'a>,
    /// Indices into `all_entries` that pass current filters.
    pub filtered_indices: Vec<usize>,

    // -- Picker popups --
    /// Whether to show the source picker popup.
    pub show_source_picker: bool,
    /// Highlighted index in the source picker list.
    pub source_picker_selected: usize,
    /// Whether to show the log file picker popup.
    pub show_log_file_picker: bool,
    /// Highlighted index in the log file picker list.
    pub log_file_picker_selected: usize,

    // -- Viewport heights --
    pub list_viewport_height: u16,
    pub detail_viewport_height: u16,
    pub source_picker_viewport_height: u16,
    pub log_file_picker_viewport_height: u16,

    /// Cursor position for the search bar (set during render, consumed by draw).
    pub search_cursor_position: Option<(u16, u16)>,
}

impl<'a> LogsState<'a> {
    pub fn new(all_entries: &'a [LogEntry<'a>]) -> Self {
        let source_filter = SourceFilter::new(all_entries);
        let log_file_filter = LogFileFilter::new(all_entries);
        let filtered_indices: Vec<usize> = (0..all_entries.len()).collect();
        let total_logs = all_entries.len();

        Self {
            list_state: TableState::new().with_selected(0),
            list_scrollbar: ScrollbarState::new(total_logs),
            select_anchor: None,
            detail_scroll: 0,
            detail_cursor: 0,
            detail_select_anchor: None,
            detail_line_count: 0,
            detail_selecting: false,
            show_detail: false,
            detail_focused: false,
            search_query: String::new(),
            search_hits: 0,
            level_filter: LevelFilter::default(),
            source_filter,
            log_file_filter,
            filtered_indices,
            show_source_picker: false,
            source_picker_selected: 0,
            show_log_file_picker: false,
            log_file_picker_selected: 0,
            list_viewport_height: 20,
            detail_viewport_height: 20,
            source_picker_viewport_height: 20,
            log_file_picker_viewport_height: 20,
            search_cursor_position: None,
        }
    }

    // -----------------------------------------------------------------------
    // Selection ranges
    // -----------------------------------------------------------------------

    /// Ordered `(start, end)` selection range for the log list.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.select_anchor?;
        let cursor = self.list_state.selected();
        cursor.map(|selected| (anchor.min(selected), anchor.max(selected)))
    }

    /// Ordered `(start, end)` selection range for the detail pane.
    pub fn detail_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.detail_select_anchor?;
        Some((
            anchor.min(self.detail_cursor),
            anchor.max(self.detail_cursor),
        ))
    }

    /// Clamp scroll so the detail cursor is visible.
    pub fn ensure_detail_cursor_visible(&mut self) {
        ensure_cursor_visible(
            self.detail_cursor,
            &mut self.detail_scroll,
            self.detail_viewport_height,
        );
    }

    // -----------------------------------------------------------------------
    // Filtering
    // -----------------------------------------------------------------------

    /// Recompute `filtered_indices` based on current filters.
    pub fn refilter(&mut self, all_entries: &[LogEntry<'_>]) {
        self.refilter_inner(all_entries, None);
    }

    pub(crate) fn refilter_inner(
        &mut self,
        all_entries: &[LogEntry<'_>],
        pinned_all_entry_idx: Option<usize>,
    ) {
        self.filtered_indices = all_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                self.level_filter.accepts(entry.level)
                    && self.source_filter.accepts(entry)
                    && self.log_file_filter.accepts(entry)
            })
            .map(|(i, _)| i)
            .collect();

        if let Some(pinned) = pinned_all_entry_idx {
            self.list_state
                .select(self.filtered_indices.iter().position(|&idx| idx == pinned));
        } else {
            let Some(selected) = self.list_state.selected() else {
                return;
            };
            if !self.filtered_indices.is_empty() {
                if selected >= self.filtered_indices.len() {
                    self.list_state
                        .select(Some(self.filtered_indices.len() - 1));
                }
            } else {
                self.list_state.select(Some(0))
            }
        }
        self.detail_scroll = 0;
    }

    /// Move the cursor forward to the next entry matching the search query.
    pub fn find_next(&mut self, all_entries: &[LogEntry<'_>]) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            return;
        }
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        let query_lower = self.search_query.to_lowercase();
        let len = self.filtered_indices.len();
        for offset in 1..=len {
            let pos = (selected + offset) % len;
            if entry_matches_query(all_entries, self.filtered_indices[pos], &query_lower) {
                self.list_state.select(Some(pos));
                self.detail_scroll = 0;
                return;
            }
        }
    }

    /// Move the cursor backward to the previous entry matching the search query.
    pub fn find_prev(&mut self, all_entries: &[LogEntry<'_>]) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            return;
        }
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        let query_lower = self.search_query.to_lowercase();
        let len = self.filtered_indices.len();
        for offset in 1..=len {
            let pos = (selected + len - offset) % len;
            if entry_matches_query(all_entries, self.filtered_indices[pos], &query_lower) {
                self.list_state.select(Some(pos));
                self.detail_scroll = 0;
                return;
            }
        }
    }

    /// Move the cursor to the nearest matching entry at or after the current
    /// position. Used for live search-as-you-type.
    pub fn find_nearest(&mut self, all_entries: &[LogEntry<'_>]) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        let len = self.filtered_indices.len();
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        for offset in 0..len {
            let pos = (selected + offset) % len;
            if entry_matches_query(all_entries, self.filtered_indices[pos], &query_lower) {
                self.list_state.select(Some(pos));
                self.detail_scroll = 0;
                return;
            }
        }
    }

    pub fn compute_search_hits(&mut self, all_entries: &'a [LogEntry<'a>]) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            self.search_hits = 0;
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        self.search_hits = all_entries
            .iter()
            .filter(|entry| contains_case_insensitive(entry.message, &query_lower))
            .count();
    }
}

// -----------------------------------------------------------------------
// Search navigation
// -----------------------------------------------------------------------

fn contains_case_insensitive(target: &str, search: &str) -> bool {
    // search is already lowercased once by the caller.
    let search = search.as_bytes();
    target
        .as_bytes()
        .windows(search.len())
        .any(|window| window.eq_ignore_ascii_case(search))
}

/// Returns `true` if the entry at `all_entries[idx]` matches the current
/// search query (case-insensitive substring in message or continuation).
fn entry_matches_query(all_entries: &[LogEntry<'_>], idx: usize, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return false;
    }
    let entry = &all_entries[idx];
    contains_case_insensitive(entry.message, query_lower)
        || entry
            .continuation
            .iter()
            .any(|c| contains_case_insensitive(c, query_lower))
}
