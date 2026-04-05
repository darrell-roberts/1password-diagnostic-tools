//! Navigation helpers: directional movement, paging, and viewport scrolling.
//!
//! These methods live in their own module to keep the main `App` impl focused
//! on construction and high-level dispatch.

use super::{
    App,
    state::{ContainerStateExt, Tab},
};

/// Clamp `scroll` so that `cursor` is visible within a viewport of `viewport_h` rows.
pub(crate) fn ensure_cursor_visible(cursor: usize, scroll: &mut u16, viewport_h: u16) {
    let scroll_usize = *scroll as usize;
    let horizontal = viewport_h as usize;
    if cursor < scroll_usize {
        *scroll = cursor as u16;
    } else if horizontal > 0 && cursor >= scroll_usize + horizontal {
        *scroll = (cursor - horizontal + 1) as u16;
    }
}

impl App<'_> {
    // -----------------------------------------------------------------------
    // Directional navigation (up / down / page-up / page-down / home / end)
    // -----------------------------------------------------------------------

    pub(crate) fn navigate_up(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview.cursor > 0 {
                    self.overview.cursor -= 1;
                    self.overview.ensure_cursor_visible();
                }
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    if self.logs.detail_cursor > 0 {
                        self.logs.detail_cursor -= 1;
                        self.logs.ensure_detail_cursor_visible();
                    }
                } else {
                    self.logs.list_state.up();
                    self.logs.detail_scroll = 0;
                    self.logs.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    if self.crashes.detail_cursor > 0 {
                        self.crashes.detail_cursor -= 1;
                        self.crashes.ensure_detail_cursor_visible();
                    }
                } else {
                    self.crashes.list_state.up();
                    self.crashes.detail_scroll = 0;
                    self.crashes.detail_cursor = 0;
                }
            }
            Tab::Analysis => {
                if self.analysis.cursor > 0 {
                    self.analysis.cursor -= 1;
                    self.analysis.ensure_cursor_visible();
                }
            }
        }
    }

    pub(crate) fn navigate_down(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview.line_count > 0
                    && self.overview.cursor + 1 < self.overview.line_count
                {
                    self.overview.cursor += 1;
                    self.overview.ensure_cursor_visible();
                }
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    if self.logs.detail_line_count > 0
                        && self.logs.detail_cursor + 1 < self.logs.detail_line_count
                    {
                        self.logs.detail_cursor += 1;
                        self.logs.ensure_detail_cursor_visible();
                    }
                } else {
                    self.logs.list_state.down();
                    self.logs.detail_scroll = 0;
                    self.logs.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    if self.crashes.detail_line_count > 0
                        && self.crashes.detail_cursor + 1 < self.crashes.detail_line_count
                    {
                        self.crashes.detail_cursor += 1;
                        self.crashes.ensure_detail_cursor_visible();
                    }
                } else {
                    self.crashes.list_state.down();
                    self.crashes.detail_scroll = 0;
                    self.crashes.detail_cursor = 0;
                }
            }
            Tab::Analysis => {
                if self.analysis.line_count > 0
                    && self.analysis.cursor + 1 < self.analysis.line_count
                {
                    self.analysis.cursor += 1;
                    self.analysis.ensure_cursor_visible();
                }
            }
        }
    }

    pub(crate) fn navigate_page_up(&mut self) {
        match self.tab {
            Tab::Overview => {
                let page = self.overview.viewport_height as usize;
                self.overview.cursor = self.overview.cursor.saturating_sub(page);
                self.overview.ensure_cursor_visible();
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    let page = self.logs.detail_viewport_height as usize;
                    self.logs.detail_cursor = self.logs.detail_cursor.saturating_sub(page);
                    self.logs.ensure_detail_cursor_visible();
                } else {
                    let page = self.logs.list_viewport_height;
                    self.logs.list_state.page_up(page);
                    self.logs.detail_scroll = 0;
                    self.logs.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    let page = self.crashes.detail_viewport_height as usize;
                    self.crashes.detail_cursor = self.crashes.detail_cursor.saturating_sub(page);
                    self.crashes.ensure_detail_cursor_visible();
                } else {
                    let page = self.crashes.list_viewport_height;
                    self.crashes.list_state.page_up(page);
                    self.crashes.detail_scroll = 0;
                    self.crashes.detail_cursor = 0;
                }
            }
            Tab::Analysis => {
                let page = self.analysis.viewport_height as usize;
                self.analysis.cursor = self.analysis.cursor.saturating_sub(page);
                self.analysis.ensure_cursor_visible();
            }
        }
    }

    pub(crate) fn navigate_page_down(&mut self) {
        match self.tab {
            Tab::Overview => {
                let page = self.overview.viewport_height as usize;
                if self.overview.line_count > 0 {
                    self.overview.cursor =
                        (self.overview.cursor + page).min(self.overview.line_count - 1);
                }
                self.overview.ensure_cursor_visible();
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    let page = self.logs.detail_viewport_height;
                    if self.logs.detail_line_count > 0 {
                        self.logs.detail_cursor = (self.logs.detail_cursor + page as usize)
                            .min(self.logs.detail_line_count - 1);
                    }
                    self.logs.ensure_detail_cursor_visible();
                } else {
                    let page = self.logs.list_viewport_height;
                    self.logs.list_state.page_down(page);
                    self.logs.detail_scroll = 0;
                    self.logs.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    let page = self.crashes.detail_viewport_height as usize;
                    if self.crashes.detail_line_count > 0 {
                        self.crashes.detail_cursor = (self.crashes.detail_cursor + page)
                            .min(self.crashes.detail_line_count - 1);
                    }
                    self.crashes.ensure_detail_cursor_visible();
                } else {
                    let page = self.crashes.list_viewport_height;
                    self.crashes.list_state.page_down(page);
                    self.crashes.detail_scroll = 0;
                }
            }
            Tab::Analysis => {
                let page = self.analysis.viewport_height as usize;
                if self.analysis.line_count > 0 {
                    self.analysis.cursor =
                        (self.analysis.cursor + page).min(self.analysis.line_count - 1);
                }
                self.analysis.ensure_cursor_visible();
            }
        }
    }

    pub(crate) fn navigate_home(&mut self) {
        match self.tab {
            Tab::Overview => {
                self.overview.cursor = 0;
                self.overview.ensure_cursor_visible();
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    self.logs.detail_cursor = 0;
                    self.logs.ensure_detail_cursor_visible();
                } else {
                    self.logs.list_state.home();
                    self.logs.detail_scroll = 0;
                    self.logs.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    self.crashes.detail_cursor = 0;
                    self.crashes.ensure_detail_cursor_visible();
                } else {
                    self.crashes.list_state.home();
                    self.crashes.detail_scroll = 0;
                    self.crashes.detail_cursor = 0;
                }
            }
            Tab::Analysis => {
                self.analysis.cursor = 0;
                self.analysis.ensure_cursor_visible();
            }
        }
    }

    pub(crate) fn navigate_end(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview.line_count > 0 {
                    self.overview.cursor = self.overview.line_count - 1;
                }
                self.overview.ensure_cursor_visible();
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    if self.logs.detail_line_count > 0 {
                        self.logs.detail_cursor = self.logs.detail_line_count - 1;
                    }
                    self.logs.ensure_detail_cursor_visible();
                } else {
                    self.logs.list_state.end();
                    self.logs.detail_scroll = 0;
                    self.logs.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    if self.crashes.detail_line_count > 0 {
                        self.crashes.detail_cursor = self.crashes.detail_line_count - 1;
                    }
                    self.crashes.ensure_detail_cursor_visible();
                } else {
                    self.crashes.list_state.end();
                    self.crashes.detail_scroll = 0;
                }
            }
            Tab::Analysis => {
                if self.analysis.line_count > 0 {
                    self.analysis.cursor = self.analysis.line_count - 1;
                }
                self.analysis.ensure_cursor_visible();
            }
        }
    }

    // -----------------------------------------------------------------------
    // z-commands: scroll the viewport so the cursor line is at the center,
    // top, or bottom of the visible area — matching vi's zz / zt / zb.
    // -----------------------------------------------------------------------

    /// Scroll viewport so the current cursor line is centered (`zz`).
    pub(crate) fn scroll_cursor_center(&mut self) {
        match self.tab {
            Tab::Overview => {
                let half = (self.overview.viewport_height as usize) / 2;
                self.overview.scroll = self.overview.cursor.saturating_sub(half) as u16;
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    let half = (self.logs.detail_viewport_height as usize) / 2;
                    self.logs.detail_scroll = self.logs.detail_cursor.saturating_sub(half) as u16;
                } else {
                    let half = (self.logs.list_viewport_height as usize) / 2;
                    let selected = self.logs.list_state.selected().unwrap_or_default();
                    *self.logs.list_state.offset_mut() = selected.saturating_sub(half);
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    let half = (self.crashes.detail_viewport_height as usize) / 2;
                    self.crashes.detail_scroll =
                        self.crashes.detail_cursor.saturating_sub(half) as u16;
                } else {
                    let half = (self.crashes.list_viewport_height as usize) / 2;
                    let selected = self.crashes.list_state.selected().unwrap_or_default();
                    *self.crashes.list_state.offset_mut() = selected.saturating_sub(half);
                }
            }
            Tab::Analysis => {
                let half = (self.analysis.viewport_height as usize) / 2;
                self.analysis.scroll = self.analysis.cursor.saturating_sub(half) as u16;
            }
        }
    }

    /// Scroll viewport so the current cursor line is at the top (`zt`).
    pub(crate) fn scroll_cursor_top(&mut self) {
        match self.tab {
            Tab::Overview => {
                self.overview.scroll = self.overview.cursor as u16;
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    self.logs.detail_scroll = self.logs.detail_cursor as u16;
                } else {
                    let selected = self.logs.list_state.selected().unwrap_or_default();
                    *self.logs.list_state.offset_mut() = selected;
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    self.crashes.detail_scroll = self.crashes.detail_cursor as u16;
                } else {
                    let selected = self.crashes.list_state.selected().unwrap_or(0);
                    *self.crashes.list_state.offset_mut() = selected;
                }
            }
            Tab::Analysis => {
                self.analysis.scroll = self.analysis.cursor as u16;
            }
        }
    }

    /// Scroll viewport so the current cursor line is at the bottom (`zb`).
    pub(crate) fn scroll_cursor_bottom(&mut self) {
        match self.tab {
            Tab::Overview => {
                let height = self.overview.viewport_height as usize;
                self.overview.scroll = (self.overview.cursor + 1).saturating_sub(height) as u16;
            }
            Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    let height = self.logs.detail_viewport_height as usize;
                    self.logs.detail_scroll =
                        (self.logs.detail_cursor + 1).saturating_sub(height) as u16;
                } else {
                    let height = self.logs.list_viewport_height as usize;
                    let selected = self.logs.list_state.selected().unwrap_or_default();
                    *self.logs.list_state.offset_mut() = (selected + 1).saturating_sub(height);
                }
            }
            Tab::CrashReports => {
                if self.crashes.detail_focused {
                    let height = self.crashes.detail_viewport_height as usize;
                    self.crashes.detail_scroll =
                        (self.crashes.detail_cursor + 1).saturating_sub(height) as u16;
                } else {
                    let height = self.crashes.list_viewport_height as usize;
                    let selected = self.crashes.list_state.selected().unwrap_or_default();
                    *self.crashes.list_state.offset_mut() = (selected + 1).saturating_sub(height);
                }
            }
            Tab::Analysis => {
                let height = self.analysis.viewport_height as usize;
                self.analysis.scroll = (self.analysis.cursor + 1).saturating_sub(height) as u16;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Mouse scroll
    // -----------------------------------------------------------------------

    /// Handle mouse scroll-up events.
    pub fn handle_scroll_up(&mut self) {
        // Dismiss help overlay on any scroll.
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Scroll inside the source picker when it is open.
        if self.logs.show_source_picker {
            if self.logs.source_picker_selected > 0 {
                self.logs.source_picker_selected -= 1;
            }
            return;
        }

        // Scroll inside the log file picker when it is open.
        if self.logs.show_log_file_picker {
            if self.logs.log_file_picker_selected > 0 {
                self.logs.log_file_picker_selected -= 1;
            }
            return;
        }

        // Scroll 3 lines at a time for a comfortable feel.
        for _ in 0..3 {
            self.navigate_up();
        }
    }

    /// Handle mouse scroll-down events.
    pub fn handle_scroll_down(&mut self) {
        // Dismiss help overlay on any scroll.
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Scroll inside the source picker when it is open.
        if self.logs.show_source_picker {
            let total = 1 + self.logs.source_filter.available.len();
            if self.logs.source_picker_selected + 1 < total {
                self.logs.source_picker_selected += 1;
            }
            return;
        }

        // Scroll inside the log file picker when it is open.
        if self.logs.show_log_file_picker {
            let total = 1 + self.logs.log_file_filter.available.len();
            if self.logs.log_file_picker_selected + 1 < total {
                self.logs.log_file_picker_selected += 1;
            }
            return;
        }

        // Scroll 3 lines at a time for a comfortable feel.
        for _ in 0..3 {
            self.navigate_down();
        }
    }

    // -----------------------------------------------------------------------
    // Tab navigation guard
    // -----------------------------------------------------------------------

    pub(crate) fn tab_nav_keys(&self) -> bool {
        true
    }
}
