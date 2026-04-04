//! State for the Overview tab.

use super::navigation::ensure_cursor_visible;

/// Persistent state for the Overview tab.
pub struct OverviewState {
    /// Scroll offset.
    pub scroll: u16,
    /// Cursor line index.
    pub cursor: usize,
    /// Anchor line for visual selection (`None` when not selecting).
    pub select_anchor: Option<usize>,
    /// Total number of content lines (set during rendering).
    pub line_count: usize,
    /// Last-known viewport height in rows.
    pub viewport_height: u16,
}

impl Default for OverviewState {
    fn default() -> Self {
        Self {
            scroll: 0,
            cursor: 0,
            select_anchor: None,
            line_count: 0,
            viewport_height: 20,
        }
    }
}

impl OverviewState {
    /// Ordered `(start, end)` selection range, if in select mode.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.select_anchor?;
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// Clamp scroll so the cursor is visible.
    pub fn ensure_cursor_visible(&mut self) {
        ensure_cursor_visible(self.cursor, &mut self.scroll, self.viewport_height);
    }
}
