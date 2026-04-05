//! Keyboard input handling for each application mode.
//!
//! All `handle_*_key` methods live here to keep the main `App` impl focused
//! on construction and high-level dispatch. Each method returns `true` when
//! the application should quit.

use crate::app::{
    App,
    navigation::ensure_cursor_visible,
    state::{ContainerStateExt as _, InputMode, Tab},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Which table-backed list is active in visual-select mode.
pub(super) enum ListSelectTarget {
    Logs,
    Crashes,
}

/// Which cursor-based pane is active in visual-select mode.
pub(super) enum PaneSelectTarget {
    Overview,
    LogDetail,
    CrashDetail,
}

impl App<'_> {
    /// Handle keys when in search input mode.
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.logs.search_query.clear();
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.logs.search_query.pop();
                self.logs.find_nearest(self.all_entries);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.logs.search_query.clear();
            }
            KeyCode::Char(c) => {
                self.logs.search_query.push(c);
                self.logs.find_nearest(self.all_entries);
            }
            _ => {}
        }
        false
    }

    /// Handle keys when in normal navigation mode.
    pub(super) fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        // Clear the "Copied!" flash after a short time on any keypress.
        if self
            .copied_at
            .is_some_and(|t| t.elapsed().as_millis() > 300)
        {
            self.copied_at = None;
        }

        // Handle second key of a two-key `z` command.
        if self.pending_z {
            self.pending_z = false;
            match key.code {
                KeyCode::Char('z') => self.scroll_cursor_center(),
                KeyCode::Char('t') => self.scroll_cursor_top(),
                KeyCode::Char('b') => self.scroll_cursor_bottom(),
                _ => {}
            }
            return false;
        }

        let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            // Quit.
            KeyCode::Char('q') => return true,

            // Help.
            KeyCode::Char('?') => {
                self.show_help = true;
            }

            // Tab switching.
            KeyCode::Tab | KeyCode::Right if key.modifiers.is_empty() && self.tab_nav_keys() => {
                if key.code == KeyCode::Tab {
                    self.tab = self.tab.next();
                    self.logs.detail_focused = false;
                    self.crashes.detail_focused = false;
                    self.logs.show_detail = false;
                }
            }
            KeyCode::BackTab => {
                self.tab = self.tab.prev();
                self.logs.detail_focused = false;
                self.crashes.detail_focused = false;
                self.logs.show_detail = false;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::Overview;
                self.logs.detail_focused = false;
                self.crashes.detail_focused = false;
                self.logs.show_detail = false;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Logs;
                self.logs.detail_focused = false;
                self.crashes.detail_focused = false;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::CrashReports;
                self.logs.detail_focused = false;
                self.crashes.detail_focused = false;
                self.logs.show_detail = false;
            }

            // Search (only on Logs tab).
            KeyCode::Char('/') if self.tab == Tab::Logs => {
                self.input_mode = InputMode::Search;
            }

            // Clear search / close log detail / unfocus detail.
            KeyCode::Esc => {
                if self.tab == Tab::Logs && self.logs.show_detail && self.logs.detail_focused {
                    self.logs.detail_focused = false;
                } else if self.tab == Tab::Logs && self.logs.show_detail {
                    self.logs.show_detail = false;
                    self.logs.detail_scroll = 0;
                } else if !self.logs.search_query.is_empty() {
                    self.logs.search_query.clear();
                } else {
                    self.logs.detail_focused = false;
                    self.crashes.detail_focused = false;
                }
            }

            // Find next / previous match.
            KeyCode::Char('n') if self.tab == Tab::Logs && !self.logs.search_query.is_empty() => {
                self.logs.find_next(self.all_entries);
            }
            KeyCode::Char('N') if self.tab == Tab::Logs && !self.logs.search_query.is_empty() => {
                self.logs.find_prev(self.all_entries);
            }

            // Level filter cycle.
            KeyCode::Char('f') if self.tab == Tab::Logs && !control_pressed => {
                self.logs.level_filter.cycle();
                self.logs.refilter(self.all_entries);
            }

            KeyCode::Char('f') if control_pressed => {
                self.navigate_page_down();
            }

            KeyCode::Char('u') if control_pressed => {
                self.navigate_page_up();
            }

            // Source filter cycle.
            KeyCode::Char('s') if self.tab == Tab::Logs => {
                self.logs.source_filter.cycle_next();
                self.logs.refilter(self.all_entries);
            }

            // Source picker popup.
            KeyCode::Char('S') if self.tab == Tab::Logs => {
                // Sync picker selection with current filter state.
                self.logs.source_picker_selected = match self.logs.source_filter.selected {
                    None => 0,
                    Some(idx) => idx + 1,
                };
                self.logs.show_source_picker = true;
            }

            // Reset source filter to All Sources.
            KeyCode::Char('a') if self.tab == Tab::Logs => {
                self.logs.source_filter.selected = None;
                self.logs.refilter(self.all_entries);
            }

            // Log file filter cycle.
            KeyCode::Char('l') if self.tab == Tab::Logs => {
                self.logs.log_file_filter.cycle_next();
                self.logs.refilter(self.all_entries);
            }

            // Log file picker popup.
            KeyCode::Char('L') if self.tab == Tab::Logs => {
                self.logs.log_file_picker_selected = match self.logs.log_file_filter.selected {
                    None => 0,
                    Some(idx) => idx + 1,
                };
                self.logs.show_log_file_picker = true;
            }

            // Reset log file filter to All Log Files (combine all logs).
            KeyCode::Char('A') if self.tab == Tab::Logs => {
                self.logs.log_file_filter.selected = None;
                self.logs.refilter(self.all_entries);
            }

            // Toggle detail view.
            KeyCode::Char('d') | KeyCode::Enter => {
                if self.tab == Tab::Logs {
                    if self.logs.show_detail && self.logs.detail_focused {
                        self.logs.detail_focused = false;
                    } else if self.logs.show_detail && !self.logs.detail_focused {
                        self.logs.detail_focused = true;
                        self.logs.detail_cursor = 0;
                        self.logs.detail_scroll = 0;
                    } else {
                        self.logs.show_detail = true;
                        self.logs.detail_focused = true;
                        self.logs.detail_cursor = 0;
                        self.logs.detail_scroll = 0;
                    }
                } else if self.tab == Tab::CrashReports {
                    self.crashes.detail_focused = !self.crashes.detail_focused;
                    self.crashes.detail_cursor = 0;
                    self.crashes.detail_scroll = 0;
                }
            }

            // Start a two-key z command (zz, zt, zb).
            KeyCode::Char('z') => {
                self.pending_z = true;
            }

            // Navigation.
            KeyCode::Up | KeyCode::Char('k') => self.navigate_up(),
            KeyCode::Down | KeyCode::Char('j') => self.navigate_down(),
            KeyCode::PageUp => self.navigate_page_up(),
            KeyCode::PageDown => self.navigate_page_down(),
            KeyCode::Home | KeyCode::Char('g') => self.navigate_home(),
            KeyCode::End | KeyCode::Char('G') => self.navigate_end(),

            // Visual select mode (Overview tab).
            KeyCode::Char('v') if self.tab == Tab::Overview => {
                self.overview.select_anchor = Some(self.overview.cursor);
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Logs tab — detail pane focused).
            KeyCode::Char('v')
                if self.tab == Tab::Logs && self.logs.show_detail && self.logs.detail_focused =>
            {
                self.logs.detail_select_anchor = Some(self.logs.detail_cursor);
                self.logs.detail_selecting = true;
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Logs tab — list focused).
            KeyCode::Char('v') if self.tab == Tab::Logs => {
                self.logs.select_anchor = self.logs.list_state.selected();
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Crash Reports — detail pane focused).
            KeyCode::Char('v') if self.tab == Tab::CrashReports && self.crashes.detail_focused => {
                self.crashes.detail_select_anchor = Some(self.crashes.detail_cursor);
                self.crashes.detail_selecting = true;
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Crash Reports list — list focused).
            KeyCode::Char('v') if self.tab == Tab::CrashReports => {
                self.crashes.select_anchor = self.crashes.list_state.selected();
                self.input_mode = InputMode::Select;
            }

            // Copy single line under cursor (Overview tab).
            KeyCode::Char('y') if self.tab == Tab::Overview => {
                self.overview.cursor = self.overview.scroll as usize;
                self.overview.select_anchor = Some(self.overview.cursor);
                self.copy_overview_selection();
            }

            // Copy single line under cursor (Logs tab — detail pane focused).
            KeyCode::Char('y')
                if self.tab == Tab::Logs && self.logs.show_detail && self.logs.detail_focused =>
            {
                self.logs.detail_select_anchor = Some(self.logs.detail_cursor);
                self.logs.detail_selecting = true;
                self.copy_detail_selection();
            }

            // Copy single entry under cursor (Logs tab — list focused).
            KeyCode::Char('y') if self.tab == Tab::Logs => {
                self.logs.select_anchor = self.logs.list_state.selected();
                self.copy_selection();
            }

            // Copy single line under cursor (Crash Reports — detail pane focused).
            KeyCode::Char('y') if self.tab == Tab::CrashReports && self.crashes.detail_focused => {
                self.crashes.detail_select_anchor = Some(self.crashes.detail_cursor);
                self.crashes.detail_selecting = true;
                self.copy_crash_detail_selection();
            }

            // Copy crash entry (Crash Reports — list focused).
            KeyCode::Char('y') if self.tab == Tab::CrashReports => {
                self.copy_crash_selection();
            }

            // Right arrow to open/focus detail, left arrow to close/unfocus it.
            KeyCode::Right if self.tab == Tab::Logs => {
                if !self.logs.show_detail {
                    self.logs.show_detail = true;
                    self.logs.detail_focused = true;
                    self.logs.detail_cursor = 0;
                    self.logs.detail_scroll = 0;
                } else if !self.logs.detail_focused {
                    self.logs.detail_focused = true;
                    self.logs.detail_cursor = 0;
                    self.logs.detail_scroll = 0;
                }
            }
            KeyCode::Right if self.tab == Tab::CrashReports => {
                self.crashes.detail_focused = true;
                self.crashes.detail_cursor = 0;
                self.crashes.detail_scroll = 0;
            }
            KeyCode::Left if self.tab == Tab::Logs => {
                if self.logs.show_detail && self.logs.detail_focused {
                    self.logs.detail_focused = false;
                } else if self.logs.show_detail {
                    self.logs.show_detail = false;
                    self.logs.detail_scroll = 0;
                }
            }
            KeyCode::Left if self.tab == Tab::CrashReports => {
                self.crashes.detail_focused = false;
            }

            _ => {}
        }
        false
    }

    /// Handle keys while in visual-select mode on a table-backed list
    /// (Logs list or Crash list).
    pub(super) fn handle_list_select_key(
        &mut self,
        key: KeyEvent,
        target: ListSelectTarget,
    ) -> bool {
        if self.pending_z {
            self.pending_z = false;
            match key.code {
                KeyCode::Char('z') => self.scroll_cursor_center(),
                KeyCode::Char('t') => self.scroll_cursor_top(),
                KeyCode::Char('b') => self.scroll_cursor_bottom(),
                _ => {}
            }
            return false;
        }

        let (table, page, scroll_reset) = match target {
            ListSelectTarget::Logs => (
                &mut self.logs.list_state,
                self.logs.list_viewport_height,
                &mut self.logs.detail_scroll,
            ),
            ListSelectTarget::Crashes => (
                &mut self.crashes.list_state,
                self.crashes.list_viewport_height,
                &mut self.crashes.detail_scroll,
            ),
        };

        match key.code {
            KeyCode::Esc => {}
            KeyCode::Char('y') => {}
            KeyCode::Up | KeyCode::Char('k') => {
                table.up();
                *scroll_reset = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                table.down();
                *scroll_reset = 0;
            }
            KeyCode::PageUp => {
                table.page_up(page);
                *scroll_reset = 0;
            }
            KeyCode::PageDown => {
                table.page_down(page);
                *scroll_reset = 0;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                table.home();
                *scroll_reset = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                table.end();
                *scroll_reset = 0;
            }
            KeyCode::Char('z') => {
                self.pending_z = true;
            }
            _ => {}
        }

        // Handle Esc/y after the borrow on table/scroll_reset is released.
        match key.code {
            KeyCode::Esc => {
                match target {
                    ListSelectTarget::Logs => self.logs.select_anchor = None,
                    ListSelectTarget::Crashes => self.crashes.select_anchor = None,
                }
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char('y') => match target {
                ListSelectTarget::Logs => self.copy_selection(),
                ListSelectTarget::Crashes => self.copy_crash_selection(),
            },
            _ => {}
        }
        false
    }

    /// Handle keys while in visual-select mode on a cursor-based pane
    /// (Overview, Log detail, or Crash detail).
    pub(super) fn handle_pane_select_key(
        &mut self,
        key: KeyEvent,
        target: PaneSelectTarget,
    ) -> bool {
        let (cursor, scroll, line_count, viewport_horizontal) = match target {
            PaneSelectTarget::Overview => (
                &mut self.overview.cursor,
                &mut self.overview.scroll,
                self.overview.line_count,
                self.overview.viewport_height,
            ),
            PaneSelectTarget::LogDetail => (
                &mut self.logs.detail_cursor,
                &mut self.logs.detail_scroll,
                self.logs.detail_line_count,
                self.logs.detail_viewport_height,
            ),
            PaneSelectTarget::CrashDetail => (
                &mut self.crashes.detail_cursor,
                &mut self.crashes.detail_scroll,
                self.crashes.detail_line_count,
                self.crashes.detail_viewport_height,
            ),
        };

        if self.pending_z {
            self.pending_z = false;
            let half = (viewport_horizontal as usize) / 2;
            match key.code {
                KeyCode::Char('z') => *scroll = cursor.saturating_sub(half) as u16,
                KeyCode::Char('t') => *scroll = *cursor as u16,
                KeyCode::Char('b') => {
                    *scroll = (*cursor + 1).saturating_sub(viewport_horizontal as usize) as u16
                }
                _ => {}
            }
            return false;
        }

        let page = viewport_horizontal as usize;
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Char('y') => {}
            KeyCode::Up | KeyCode::Char('k') => {
                if *cursor > 0 {
                    *cursor -= 1;
                    ensure_cursor_visible(*cursor, scroll, viewport_horizontal);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if line_count > 0 && *cursor + 1 < line_count {
                    *cursor += 1;
                    ensure_cursor_visible(*cursor, scroll, viewport_horizontal);
                }
            }
            KeyCode::PageUp => {
                *cursor = cursor.saturating_sub(page);
                ensure_cursor_visible(*cursor, scroll, viewport_horizontal);
            }
            KeyCode::PageDown => {
                if line_count > 0 {
                    *cursor = (*cursor + page).min(line_count - 1);
                }
                ensure_cursor_visible(*cursor, scroll, viewport_horizontal);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                *cursor = 0;
                ensure_cursor_visible(*cursor, scroll, viewport_horizontal);
            }
            KeyCode::End | KeyCode::Char('G') => {
                if line_count > 0 {
                    *cursor = line_count - 1;
                }
                ensure_cursor_visible(*cursor, scroll, viewport_horizontal);
            }
            KeyCode::Char('z') => {
                self.pending_z = true;
            }
            _ => {}
        }

        // Handle Esc/y after the borrow on cursor/scroll is released.
        match key.code {
            KeyCode::Esc => {
                match target {
                    PaneSelectTarget::Overview => self.overview.select_anchor = None,
                    PaneSelectTarget::LogDetail => {
                        self.logs.detail_select_anchor = None;
                        self.logs.detail_selecting = false;
                    }
                    PaneSelectTarget::CrashDetail => {
                        self.crashes.detail_select_anchor = None;
                        self.crashes.detail_selecting = false;
                    }
                }
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char('y') => match target {
                PaneSelectTarget::Overview => self.copy_overview_selection(),
                PaneSelectTarget::LogDetail => self.copy_detail_selection(),
                PaneSelectTarget::CrashDetail => self.copy_crash_detail_selection(),
            },
            _ => {}
        }
        false
    }

    // -----------------------------------------------------------------------
    // Popup picker handlers
    // -----------------------------------------------------------------------

    /// Handle keys when the source picker popup is open.
    pub(super) fn handle_source_picker_key(&mut self, key: KeyEvent) -> bool {
        let total = 1 + self.logs.source_filter.available.len();
        let page = self.logs.source_picker_viewport_height as usize;

        match key.code {
            KeyCode::Esc | KeyCode::Char('S') | KeyCode::Char('s') => {
                self.logs.show_source_picker = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.logs.source_picker_selected > 0 {
                    self.logs.source_picker_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.logs.source_picker_selected + 1 < total {
                    self.logs.source_picker_selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.logs.source_picker_selected =
                    self.logs.source_picker_selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if total > 0 {
                    self.logs.source_picker_selected =
                        (self.logs.source_picker_selected + page).min(total - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.logs.source_picker_selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if total > 0 {
                    self.logs.source_picker_selected = total - 1;
                }
            }
            KeyCode::Enter => {
                if self.logs.source_picker_selected == 0 {
                    self.logs.source_filter.selected = None;
                } else {
                    self.logs.source_filter.selected = Some(self.logs.source_picker_selected - 1);
                }
                self.logs.show_source_picker = false;
                self.logs.refilter(self.all_entries);
            }
            _ => {}
        }
        false
    }

    /// Handle keys when the log file picker popup is open.
    pub(super) fn handle_log_file_picker_key(&mut self, key: KeyEvent) -> bool {
        let total = 1 + self.logs.log_file_filter.available.len();
        let page = self.logs.log_file_picker_viewport_height as usize;

        match key.code {
            KeyCode::Esc | KeyCode::Char('L') | KeyCode::Char('l') => {
                self.logs.show_log_file_picker = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.logs.log_file_picker_selected > 0 {
                    self.logs.log_file_picker_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.logs.log_file_picker_selected + 1 < total {
                    self.logs.log_file_picker_selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.logs.log_file_picker_selected =
                    self.logs.log_file_picker_selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if total > 0 {
                    self.logs.log_file_picker_selected =
                        (self.logs.log_file_picker_selected + page).min(total - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.logs.log_file_picker_selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if total > 0 {
                    self.logs.log_file_picker_selected = total - 1;
                }
            }
            KeyCode::Enter => {
                if self.logs.log_file_picker_selected == 0 {
                    self.logs.log_file_filter.selected = None;
                } else {
                    self.logs.log_file_filter.selected =
                        Some(self.logs.log_file_picker_selected - 1);
                }
                self.logs.show_log_file_picker = false;
                self.logs.refilter(self.all_entries);
            }
            _ => {}
        }
        false
    }
}
