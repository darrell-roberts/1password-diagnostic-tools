//! Shared scrollable pane state used by the Overview and Analysis tabs.
use super::navigation::ensure_cursor_visible;

/// Reusable scrollable pane state shared by Overview, Analysis, and any
/// future cursor based single pane tabs.
pub struct ScrollablePaneState {
    /// Scroll offset.
    pub scroll: u16,
    /// Cursor line index.
    pub cursor: usize,
    /// Anchor line for visual selection (`None` when not selecting).
    pub select_anchor: Option<usize>,
    /// Total number of content lines (set during rendering).
    pub line_count: usize,
    /// Last known viewport height in rows.
    pub viewport_height: u16,
}

impl Default for ScrollablePaneState {
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

impl ScrollablePaneState {
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

/// Persistent state for the Overview tab.
pub type OverviewState = ScrollablePaneState;
