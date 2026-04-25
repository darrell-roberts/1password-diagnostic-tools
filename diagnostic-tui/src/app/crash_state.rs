//! State for the Crash Reports tab.
use super::navigation::ensure_cursor_visible;
use ratatui::widgets::TableState;

/// Persistent state for the Crash Reports tab.
pub struct CrashReportsState {
    // -- List pane --
    /// Selected crash report index.
    pub list_state: TableState,
    /// Anchor index for visual selection on the crash list.
    pub select_anchor: Option<usize>,

    // -- Detail pane --
    /// Vertical scroll offset inside the crash detail pane.
    pub detail_scroll: u16,
    /// Cursor line index in the crash detail content.
    pub detail_cursor: usize,
    /// Anchor line for visual selection on the crash detail pane.
    pub detail_select_anchor: Option<usize>,
    /// Total lines in the crash detail content (set during rendering).
    pub detail_line_count: usize,
    /// Whether the crash detail pane is in select mode.
    pub detail_selecting: bool,
    /// Whether the detail pane is focused.
    pub detail_focused: bool,
    /// Cached plain-text lines for the crash detail pane.
    pub detail_plain_cache: Option<Vec<String>>,

    // -- Viewport heights --
    pub list_viewport_height: u16,
    pub detail_viewport_height: u16,
}

impl CrashReportsState {
    pub fn new(has_crashes: bool) -> Self {
        let selected = if has_crashes { Some(0) } else { None };
        Self {
            list_state: TableState::new().with_selected(selected),
            select_anchor: None,
            detail_scroll: 0,
            detail_cursor: 0,
            detail_select_anchor: None,
            detail_line_count: 0,
            detail_selecting: false,
            detail_focused: false,
            detail_plain_cache: None,
            list_viewport_height: 20,
            detail_viewport_height: 20,
        }
    }

    /// Ordered `(start, end)` selection range for the crash list.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.select_anchor?;
        let cursor = self.list_state.selected()?;
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    /// Ordered `(start, end)` selection range for the crash detail pane.
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
}
