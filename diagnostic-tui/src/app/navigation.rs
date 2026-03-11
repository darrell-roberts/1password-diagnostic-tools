//! Navigation helpers: directional movement, paging, and viewport scrolling.
//!
//! These methods live in their own module to keep the main `App` impl focused
//! on construction and high-level dispatch.

use super::App;
use super::state::Tab;

impl App {
    // -----------------------------------------------------------------------
    // Directional navigation (up / down / page-up / page-down / home / end)
    // -----------------------------------------------------------------------

    pub(crate) fn navigate_up(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview_cursor > 0 {
                    self.overview_cursor -= 1;
                    self.ensure_overview_cursor_visible();
                }
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    if self.detail_cursor > 0 {
                        self.detail_cursor -= 1;
                        self.ensure_detail_cursor_visible();
                    }
                } else {
                    self.log_list_state.up();
                    self.detail_scroll = 0;
                    self.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll = self.crash_detail_scroll.saturating_sub(1);
                } else {
                    self.crash_list_state.up();
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    pub(crate) fn navigate_down(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview_line_count > 0
                    && self.overview_cursor + 1 < self.overview_line_count
                {
                    self.overview_cursor += 1;
                    self.ensure_overview_cursor_visible();
                }
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    if self.detail_line_count > 0 && self.detail_cursor + 1 < self.detail_line_count
                    {
                        self.detail_cursor += 1;
                        self.ensure_detail_cursor_visible();
                    }
                } else {
                    let max = self.filtered_indices.len();
                    self.log_list_state.down(max);
                    self.detail_scroll = 0;
                    self.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll += 1;
                } else {
                    let max = self.report.crash_report_entries.len();
                    self.crash_list_state.down(max);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    pub(crate) fn navigate_page_up(&mut self) {
        match self.tab {
            Tab::Overview => {
                let page = self.viewport.overview as usize;
                self.overview_cursor = self.overview_cursor.saturating_sub(page);
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    let page = self.viewport.log_detail as usize;
                    self.detail_cursor = self.detail_cursor.saturating_sub(page);
                    self.ensure_detail_cursor_visible();
                } else {
                    let page = self.viewport.log_list as usize;
                    self.log_list_state.page_up(page);
                    self.detail_scroll = 0;
                    self.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    let page = self.viewport.crash_detail;
                    self.crash_detail_scroll = self.crash_detail_scroll.saturating_sub(page);
                } else {
                    let page = self.viewport.crash_list as usize;
                    self.crash_list_state.page_up(page);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    pub(crate) fn navigate_page_down(&mut self) {
        match self.tab {
            Tab::Overview => {
                let page = self.viewport.overview as usize;
                if self.overview_line_count > 0 {
                    self.overview_cursor =
                        (self.overview_cursor + page).min(self.overview_line_count - 1);
                }
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    let page = self.viewport.log_detail as usize;
                    if self.detail_line_count > 0 {
                        self.detail_cursor =
                            (self.detail_cursor + page).min(self.detail_line_count - 1);
                    }
                    self.ensure_detail_cursor_visible();
                } else {
                    let page = self.viewport.log_list as usize;
                    let max = self.filtered_indices.len();
                    self.log_list_state.page_down(page, max);
                    self.detail_scroll = 0;
                    self.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    let page = self.viewport.crash_detail;
                    self.crash_detail_scroll += page;
                } else {
                    let page = self.viewport.crash_list as usize;
                    let max = self.report.crash_report_entries.len();
                    self.crash_list_state.page_down(page, max);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    pub(crate) fn navigate_home(&mut self) {
        match self.tab {
            Tab::Overview => {
                self.overview_cursor = 0;
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    self.detail_cursor = 0;
                    self.ensure_detail_cursor_visible();
                } else {
                    self.log_list_state.home();
                    self.detail_scroll = 0;
                    self.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll = 0;
                } else {
                    self.crash_list_state.home();
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    pub(crate) fn navigate_end(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview_line_count > 0 {
                    self.overview_cursor = self.overview_line_count - 1;
                }
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    if self.detail_line_count > 0 {
                        self.detail_cursor = self.detail_line_count - 1;
                    }
                    self.ensure_detail_cursor_visible();
                } else {
                    let max = self.filtered_indices.len();
                    self.log_list_state.end(max);
                    self.detail_scroll = 0;
                    self.detail_cursor = 0;
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll = u16::MAX;
                } else {
                    let max = self.report.crash_report_entries.len();
                    self.crash_list_state.end(max);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // z-commands: scroll the viewport so the cursor line is at the center,
    // top, or bottom of the visible area — matching vi's zz / zt / zb.
    // -----------------------------------------------------------------------

    /// Scroll viewport so the current cursor line is centred (`zz`).
    pub(crate) fn scroll_cursor_center(&mut self) {
        match self.tab {
            Tab::Overview => {
                let half = (self.viewport.overview as usize) / 2;
                self.overview_scroll = self.overview_cursor.saturating_sub(half) as u16;
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    let half = (self.viewport.log_detail as usize) / 2;
                    self.detail_scroll = self.detail_cursor.saturating_sub(half) as u16;
                } else {
                    let half = (self.viewport.log_list as usize) / 2;
                    self.log_list_state.offset = self.log_list_state.selected.saturating_sub(half);
                }
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    // No cursor concept in the detail pane — nothing to do.
                } else {
                    let half = (self.viewport.crash_list as usize) / 2;
                    self.crash_list_state.offset =
                        self.crash_list_state.selected.saturating_sub(half);
                }
            }
        }
    }

    /// Scroll viewport so the current cursor line is at the top (`zt`).
    pub(crate) fn scroll_cursor_top(&mut self) {
        match self.tab {
            Tab::Overview => {
                self.overview_scroll = self.overview_cursor as u16;
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    self.detail_scroll = self.detail_cursor as u16;
                } else {
                    self.log_list_state.offset = self.log_list_state.selected;
                }
            }
            Tab::CrashReports => {
                if !self.detail_focused {
                    self.crash_list_state.offset = self.crash_list_state.selected;
                }
            }
        }
    }

    /// Scroll viewport so the current cursor line is at the bottom (`zb`).
    pub(crate) fn scroll_cursor_bottom(&mut self) {
        match self.tab {
            Tab::Overview => {
                let h = self.viewport.overview as usize;
                self.overview_scroll = (self.overview_cursor + 1).saturating_sub(h) as u16;
            }
            Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    let h = self.viewport.log_detail as usize;
                    self.detail_scroll = (self.detail_cursor + 1).saturating_sub(h) as u16;
                } else {
                    let h = self.viewport.log_list as usize;
                    self.log_list_state.offset =
                        (self.log_list_state.selected + 1).saturating_sub(h);
                }
            }
            Tab::CrashReports => {
                if !self.detail_focused {
                    let h = self.viewport.crash_list as usize;
                    self.crash_list_state.offset =
                        (self.crash_list_state.selected + 1).saturating_sub(h);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Cursor visibility helpers
    // -----------------------------------------------------------------------

    /// Ensure the overview cursor line is visible within the current viewport.
    pub(crate) fn ensure_overview_cursor_visible(&mut self) {
        let viewport_h = self.viewport.overview as usize;
        let scroll = self.overview_scroll as usize;
        if self.overview_cursor < scroll {
            self.overview_scroll = self.overview_cursor as u16;
        } else if viewport_h > 0 && self.overview_cursor >= scroll + viewport_h {
            self.overview_scroll = (self.overview_cursor - viewport_h + 1) as u16;
        }
    }

    /// Ensure the detail cursor line is visible within the current viewport.
    pub(crate) fn ensure_detail_cursor_visible(&mut self) {
        let viewport_h = self.viewport.log_detail as usize;
        let scroll = self.detail_scroll as usize;
        if self.detail_cursor < scroll {
            self.detail_scroll = self.detail_cursor as u16;
        } else if viewport_h > 0 && self.detail_cursor >= scroll + viewport_h {
            self.detail_scroll = (self.detail_cursor - viewport_h + 1) as u16;
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
        if self.show_source_picker {
            if self.source_picker_selected > 0 {
                self.source_picker_selected -= 1;
            }
            return;
        }

        // Scroll inside the log file picker when it is open.
        if self.show_log_file_picker {
            if self.log_file_picker_selected > 0 {
                self.log_file_picker_selected -= 1;
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
        if self.show_source_picker {
            let total = 1 + self.source_filter.available.len();
            if self.source_picker_selected + 1 < total {
                self.source_picker_selected += 1;
            }
            return;
        }

        // Scroll inside the log file picker when it is open.
        if self.show_log_file_picker {
            let total = 1 + self.log_file_filter.available.len();
            if self.log_file_picker_selected + 1 < total {
                self.log_file_picker_selected += 1;
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
        // Prevent Right arrow from being interpreted as tab-switch on Logs/Crashes.
        true
    }
}
