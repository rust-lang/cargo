//! Handle keyboard and application events for the tree.

use super::*;
use crate::ops::cargo_tree::tui::metadata::MetadataTab;

impl TuiState {
    pub(super) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.show_help {
            // Close help popup on any key press
            self.show_help = false;
        }
        if key_event.kind != KeyEventKind::Press && key_event.modifiers.is_empty() {
            return;
        }

        if self.input_mode == InputMode::Selector {
            self.handle_selector_key(key_event.code);
            return;
        }

        if self.input_mode == InputMode::Search {
            match key_event.code {
                KeyCode::Esc => {
                    self.clear_search();
                }
                KeyCode::Enter => {
                    if self.search_query.is_empty() {
                        self.clear_search();
                    } else {
                        self.input_mode = InputMode::SearchResults;
                    }
                }
                KeyCode::Backspace => {
                    if self.search_query.pop().is_none() {
                        self.clear_search();
                    } else {
                        self.request_search();
                    }
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.request_search();
                }
                _ => {}
            }
            return;
        }

        if self.show_metadata {
            match key_event.code {
                KeyCode::Tab => {
                    self.metadata_popup.tab.toggle();
                    self.metadata_popup.scroll = 0;
                    return;
                }
                KeyCode::BackTab => {
                    self.metadata_popup.tab.toggle();
                    self.metadata_popup.scroll = 0;
                    return;
                }
                _ => {}
            }
            if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                match key_event.code {
                    KeyCode::PageDown => {
                        self.metadata_popup.scroll = self
                            .metadata_popup
                            .scroll
                            .saturating_add(self.metadata_popup.page_size.max(1));
                        return;
                    }
                    KeyCode::PageUp => {
                        self.metadata_popup.scroll = self
                            .metadata_popup
                            .scroll
                            .saturating_sub(self.metadata_popup.page_size.max(1));
                        return;
                    }
                    KeyCode::Up if self.metadata_popup.tab == MetadataTab::Usage => {
                        self.metadata_popup.move_usage_selection(false);
                        self.metadata_popup.scroll = 0;
                        return;
                    }
                    KeyCode::Down if self.metadata_popup.tab == MetadataTab::Usage => {
                        self.metadata_popup.move_usage_selection(true);
                        self.metadata_popup.scroll = 0;
                        return;
                    }
                    _ => {}
                }
            }
        }

        let selected_before = self.tree_widget_state.selected_node_id();
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Esc, _) if self.input_mode == InputMode::SearchResults => {
                self.clear_search();
            }
            (KeyCode::Esc, _) if self.invert_restore.is_some() => {
                self.restore_inverted_tree();
            }
            (KeyCode::Char('q'), _) => {
                self.running = false;
            }
            (KeyCode::Char('?'), _) => {
                self.show_help = !self.show_help;
            }
            (KeyCode::Char('/'), _) => {
                self.input_mode = InputMode::Search;
            }
            (KeyCode::Char('e'), _) => {
                self.open_edge_selector();
            }
            (KeyCode::Char('t'), _) => {
                self.open_target_selector();
            }
            (KeyCode::Char('f'), _) => {
                self.open_feature_selector();
            }
            (KeyCode::Char('p'), _) => {
                self.open_package_selector();
            }
            (KeyCode::Char('m') | KeyCode::Enter, _) => {
                self.show_metadata = !self.show_metadata;
            }
            (KeyCode::Char('o'), _) => {
                self.open_selected_dependency();
            }
            (KeyCode::Char('d'), _) => {
                self.open_selected_docs_rs();
            }
            (KeyCode::Char('M'), _) => {
                self.open_format_selector();
            }
            (KeyCode::Char('c'), _) => {
                self.charset = match self.charset {
                    TreeCharset::Utf8 => TreeCharset::Ascii,
                    TreeCharset::Ascii => TreeCharset::Utf8,
                };
            }
            (KeyCode::Char('i'), _) => {
                self.invert_selected_package();
            }
            (KeyCode::Char('P'), _) => {
                self.tree_widget_state.select_parent(&self.dependency_tree);
            }
            (KeyCode::Char(']'), _) => {
                self.tree_widget_state
                    .select_next_sibling(&self.dependency_tree);
            }
            (KeyCode::Char('['), _) => {
                self.tree_widget_state
                    .select_previous_sibling(&self.dependency_tree);
            }
            (KeyCode::Down, _) => {
                self.tree_widget_state.select_next(&self.dependency_tree);
            }
            (KeyCode::Up, _) => {
                self.tree_widget_state
                    .select_previous(&self.dependency_tree);
            }
            (KeyCode::PageDown, _) => {
                self.tree_widget_state.page_down(&self.dependency_tree);
            }
            (KeyCode::PageUp, _) => {
                self.tree_widget_state.page_up(&self.dependency_tree);
            }
            (KeyCode::Char(' '), _) => {
                self.tree_widget_state.toggle(&self.dependency_tree);
            }
            (KeyCode::Right, _) => {
                self.tree_widget_state.expand(&self.dependency_tree);
            }
            (KeyCode::Left, _) => {
                self.tree_widget_state.collapse(&self.dependency_tree);
            }
            _ => {}
        }
        if self.show_metadata && self.tree_widget_state.selected_node_id() != selected_before {
            self.metadata_popup.scroll = 0;
        }
    }
}
