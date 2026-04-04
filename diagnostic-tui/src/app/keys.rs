//! Keyboard input handling for each application mode.
//!
//! All `handle_*_key` methods live here to keep the main `App` impl focused
//! on construction and high-level dispatch. Each method returns `true` when
//! the application should quit.

use super::App;
use crate::app::navigation::ensure_cursor_visible;
use crate::app::state::{ContainerStateExt as _, InputMode, Tab};
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

impl App {
    /// Handle keys when in search input mode.
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.find_nearest();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_query.clear();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.find_nearest();
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
                // Right arrow is only for tab nav on overview; on logs/crashes it
                // might be used differently. We use Tab universally.
                if key.code == KeyCode::Tab {
                    self.tab = self.tab.next();
                    self.detail_focused = false;
                    self.show_log_detail = false;
                }
            }
            KeyCode::BackTab => {
                self.tab = self.tab.prev();
                self.detail_focused = false;
                self.show_log_detail = false;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::Overview;
                self.detail_focused = false;
                self.show_log_detail = false;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Logs;
                self.detail_focused = false;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::CrashReports;
                self.detail_focused = false;
                self.show_log_detail = false;
            }

            // Search (only on Logs tab).
            KeyCode::Char('/') if self.tab == Tab::Logs => {
                self.input_mode = InputMode::Search;
            }

            // Clear search / close log detail / unfocus detail.
            KeyCode::Esc => {
                if self.tab == Tab::Logs && self.show_log_detail && self.detail_focused {
                    self.detail_focused = false;
                } else if self.tab == Tab::Logs && self.show_log_detail {
                    self.show_log_detail = false;
                    self.detail_scroll = 0;
                } else if !self.search_query.is_empty() {
                    self.search_query.clear();
                } else {
                    self.detail_focused = false;
                }
            }

            // Find next / previous match.
            KeyCode::Char('n') if self.tab == Tab::Logs && !self.search_query.is_empty() => {
                self.find_next();
            }
            KeyCode::Char('N') if self.tab == Tab::Logs && !self.search_query.is_empty() => {
                self.find_prev();
            }

            // Level filter cycle.
            KeyCode::Char('f') if self.tab == Tab::Logs && !control_pressed => {
                self.level_filter.cycle();
                self.refilter();
            }

            KeyCode::Char('f') if control_pressed => {
                self.navigate_page_down();
            }

            KeyCode::Char('u') if control_pressed => {
                self.navigate_page_up();
            }

            // Source filter cycle.
            KeyCode::Char('s') if self.tab == Tab::Logs => {
                self.source_filter.cycle_next();
                self.refilter();
            }

            // Source picker popup.
            KeyCode::Char('S') if self.tab == Tab::Logs => {
                // Sync picker selection with current filter state.
                self.source_picker_selected = match self.source_filter.selected {
                    None => 0,
                    Some(idx) => idx + 1,
                };
                self.show_source_picker = true;
            }

            // Reset source filter to All Sources.
            KeyCode::Char('a') if self.tab == Tab::Logs => {
                self.source_filter.selected = None;
                self.refilter();
            }

            // Log file filter cycle.
            KeyCode::Char('l') if self.tab == Tab::Logs => {
                self.log_file_filter.cycle_next();
                self.refilter();
            }

            // Log file picker popup.
            KeyCode::Char('L') if self.tab == Tab::Logs => {
                self.log_file_picker_selected = match self.log_file_filter.selected {
                    None => 0,
                    Some(idx) => idx + 1,
                };
                self.show_log_file_picker = true;
            }

            // Reset log file filter to All Log Files (combine all logs).
            KeyCode::Char('A') if self.tab == Tab::Logs => {
                self.log_file_filter.selected = None;
                self.refilter();
            }

            // Toggle detail view.
            KeyCode::Char('d') | KeyCode::Enter => {
                if self.tab == Tab::Logs {
                    if self.show_log_detail && self.detail_focused {
                        // Unfocus detail when pressing d/Enter while detail is focused.
                        self.detail_focused = false;
                    } else if self.show_log_detail && !self.detail_focused {
                        // Focus detail when pressing d/Enter while detail is visible but not focused.
                        self.detail_focused = true;
                        self.detail_cursor = 0;
                        self.detail_scroll = 0;
                    } else {
                        // Open detail and focus it.
                        self.show_log_detail = true;
                        self.detail_focused = true;
                        self.detail_cursor = 0;
                        self.detail_scroll = 0;
                    }
                } else if self.tab == Tab::CrashReports {
                    self.detail_focused = !self.detail_focused;
                    self.crash_detail_cursor = 0;
                    self.crash_detail_scroll = 0;
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
                self.overview_select_anchor = Some(self.overview_cursor);
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Logs tab — detail pane focused).
            KeyCode::Char('v')
                if self.tab == Tab::Logs && self.show_log_detail && self.detail_focused =>
            {
                self.detail_select_anchor = Some(self.detail_cursor);
                self.detail_selecting = true;
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Logs tab — list focused).
            KeyCode::Char('v') if self.tab == Tab::Logs => {
                self.select_anchor = self.log_list_state.selected();
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Crash Reports — detail pane focused).
            KeyCode::Char('v') if self.tab == Tab::CrashReports && self.detail_focused => {
                self.crash_detail_select_anchor = Some(self.crash_detail_cursor);
                self.crash_detail_selecting = true;
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Crash Reports list — list focused).
            KeyCode::Char('v') if self.tab == Tab::CrashReports => {
                self.crash_select_anchor = self.crash_list_state.selected();
                self.input_mode = InputMode::Select;
            }

            // Copy single line under cursor (Overview tab) — copies visible top line.
            KeyCode::Char('y') if self.tab == Tab::Overview => {
                self.overview_cursor = self.overview_scroll as usize;
                self.overview_select_anchor = Some(self.overview_cursor);
                self.copy_overview_selection();
            }

            // Copy single line under cursor (Logs tab — detail pane focused).
            KeyCode::Char('y')
                if self.tab == Tab::Logs && self.show_log_detail && self.detail_focused =>
            {
                self.detail_select_anchor = Some(self.detail_cursor);
                self.detail_selecting = true;
                self.copy_detail_selection();
            }

            // Copy single entry under cursor (Logs tab — list focused).
            KeyCode::Char('y') if self.tab == Tab::Logs => {
                self.select_anchor = self.log_list_state.selected();
                self.copy_selection();
            }

            // Copy single line under cursor (Crash Reports — detail pane focused).
            KeyCode::Char('y') if self.tab == Tab::CrashReports && self.detail_focused => {
                self.crash_detail_select_anchor = Some(self.crash_detail_cursor);
                self.crash_detail_selecting = true;
                self.copy_crash_detail_selection();
            }

            // Copy crash entry (Crash Reports — list focused).
            KeyCode::Char('y') if self.tab == Tab::CrashReports => {
                self.copy_crash_selection();
            }

            // Right arrow to open/focus detail, left arrow to close/unfocus it.
            KeyCode::Right if self.tab == Tab::Logs => {
                if !self.show_log_detail {
                    self.show_log_detail = true;
                    self.detail_focused = true;
                    self.detail_cursor = 0;
                    self.detail_scroll = 0;
                } else if !self.detail_focused {
                    self.detail_focused = true;
                    self.detail_cursor = 0;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Right if self.tab == Tab::CrashReports => {
                self.detail_focused = true;
                self.crash_detail_cursor = 0;
                self.crash_detail_scroll = 0;
            }
            KeyCode::Left if self.tab == Tab::Logs => {
                if self.show_log_detail && self.detail_focused {
                    self.detail_focused = false;
                } else if self.show_log_detail {
                    self.show_log_detail = false;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Left if self.tab == Tab::CrashReports => {
                self.detail_focused = false;
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
                &mut self.log_list_state,
                self.viewport.log_list,
                &mut self.detail_scroll,
            ),
            ListSelectTarget::Crashes => (
                &mut self.crash_list_state,
                self.viewport.crash_list,
                &mut self.crash_detail_scroll,
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
                    ListSelectTarget::Logs => self.select_anchor = None,
                    ListSelectTarget::Crashes => self.crash_select_anchor = None,
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
        let (cursor, scroll, line_count, viewport_h) = match target {
            PaneSelectTarget::Overview => (
                &mut self.overview_cursor,
                &mut self.overview_scroll,
                self.overview_line_count,
                self.viewport.overview,
            ),
            PaneSelectTarget::LogDetail => (
                &mut self.detail_cursor,
                &mut self.detail_scroll,
                self.detail_line_count,
                self.viewport.log_detail,
            ),
            PaneSelectTarget::CrashDetail => (
                &mut self.crash_detail_cursor,
                &mut self.crash_detail_scroll,
                self.crash_detail_line_count,
                self.viewport.crash_detail,
            ),
        };

        if self.pending_z {
            self.pending_z = false;
            let half = (viewport_h as usize) / 2;
            match key.code {
                KeyCode::Char('z') => *scroll = cursor.saturating_sub(half) as u16,
                KeyCode::Char('t') => *scroll = *cursor as u16,
                KeyCode::Char('b') => {
                    *scroll = (*cursor + 1).saturating_sub(viewport_h as usize) as u16
                }
                _ => {}
            }
            return false;
        }

        let page = viewport_h as usize;
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Char('y') => {}
            KeyCode::Up | KeyCode::Char('k') => {
                if *cursor > 0 {
                    *cursor -= 1;
                    ensure_cursor_visible(*cursor, scroll, viewport_h);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if line_count > 0 && *cursor + 1 < line_count {
                    *cursor += 1;
                    ensure_cursor_visible(*cursor, scroll, viewport_h);
                }
            }
            KeyCode::PageUp => {
                *cursor = cursor.saturating_sub(page);
                ensure_cursor_visible(*cursor, scroll, viewport_h);
            }
            KeyCode::PageDown => {
                if line_count > 0 {
                    *cursor = (*cursor + page).min(line_count - 1);
                }
                ensure_cursor_visible(*cursor, scroll, viewport_h);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                *cursor = 0;
                ensure_cursor_visible(*cursor, scroll, viewport_h);
            }
            KeyCode::End | KeyCode::Char('G') => {
                if line_count > 0 {
                    *cursor = line_count - 1;
                }
                ensure_cursor_visible(*cursor, scroll, viewport_h);
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
                    PaneSelectTarget::Overview => self.overview_select_anchor = None,
                    PaneSelectTarget::LogDetail => {
                        self.detail_select_anchor = None;
                        self.detail_selecting = false;
                    }
                    PaneSelectTarget::CrashDetail => {
                        self.crash_detail_select_anchor = None;
                        self.crash_detail_selecting = false;
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
        // Total items: 1 ("All Sources") + number of available sources.
        let total = 1 + self.source_filter.available.len();
        let page = self.viewport.source_picker as usize;

        match key.code {
            KeyCode::Esc | KeyCode::Char('S') | KeyCode::Char('s') => {
                self.show_source_picker = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.source_picker_selected > 0 {
                    self.source_picker_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.source_picker_selected + 1 < total {
                    self.source_picker_selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.source_picker_selected = self.source_picker_selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if total > 0 {
                    self.source_picker_selected =
                        (self.source_picker_selected + page).min(total - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.source_picker_selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if total > 0 {
                    self.source_picker_selected = total - 1;
                }
            }
            KeyCode::Enter => {
                if self.source_picker_selected == 0 {
                    self.source_filter.selected = None;
                } else {
                    self.source_filter.selected = Some(self.source_picker_selected - 1);
                }
                self.show_source_picker = false;
                self.refilter();
            }
            _ => {}
        }
        false
    }

    /// Handle keys when the log file picker popup is open.
    pub(super) fn handle_log_file_picker_key(&mut self, key: KeyEvent) -> bool {
        let total = 1 + self.log_file_filter.available.len();
        let page = self.viewport.log_file_picker as usize;

        match key.code {
            KeyCode::Esc | KeyCode::Char('L') | KeyCode::Char('l') => {
                self.show_log_file_picker = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.log_file_picker_selected > 0 {
                    self.log_file_picker_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.log_file_picker_selected + 1 < total {
                    self.log_file_picker_selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.log_file_picker_selected = self.log_file_picker_selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if total > 0 {
                    self.log_file_picker_selected =
                        (self.log_file_picker_selected + page).min(total - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.log_file_picker_selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if total > 0 {
                    self.log_file_picker_selected = total - 1;
                }
            }
            KeyCode::Enter => {
                if self.log_file_picker_selected == 0 {
                    self.log_file_filter.selected = None;
                } else {
                    self.log_file_filter.selected = Some(self.log_file_picker_selected - 1);
                }
                self.show_log_file_picker = false;
                self.refilter();
            }
            _ => {}
        }
        false
    }
}
