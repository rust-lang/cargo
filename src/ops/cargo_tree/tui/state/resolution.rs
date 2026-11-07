//! Apply resolved trees and update interactive state.

use super::*;

impl TuiState {
    pub(super) fn handle_tree_resolved(&mut self, result: TreeResolutionResult) {
        if result.generation != self.resolution_generation {
            return;
        }

        match result.result {
            Ok(dependency_tree) => {
                self.resolution_applying = false;
                if let Some(format) = self.pending_format.take() {
                    self.format = format;
                }
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_query.clear();
                self.search_running = false;
                self.tree_options = result.options;
                self.dependency_tree = dependency_tree;
                self.tree_widget_state = TreeWidgetState::default();
                self.metadata_popup.scroll = 0;
                self.metadata_popup.invalidate_usage_tree();
                if let crate::ops::cargo_tree::DisplayDepth::MaxDisplayDepth(depth) =
                    self.tree_options.resolution.display_depth
                    && depth != u32::MAX
                {
                    self.tree_widget_state
                        .open_to_dependency_depth(&self.dependency_tree, depth as usize);
                } else {
                    self.tree_widget_state.expand_all(&self.dependency_tree);
                }
                self.selector = None;
                self.selector_kind = None;
                self.input_mode = InputMode::Normal;
                if self.invert_reverting {
                    self.invert_restore = None;
                    self.invert_target = None;
                    self.invert_prompt = None;
                    self.invert_reverting = false;
                }
            }
            Err(error) => {
                self.resolution_applying = false;
                self.invert_reverting = false;
                if self.invert_restore.is_some()
                    && let Some(target) = &self.invert_target
                {
                    self.invert_prompt = Some(format!("{target} ({error})"));
                }
                if let Some(selector) = self.selector.as_mut() {
                    selector.applying = false;
                    selector.message = Some(error);
                }
            }
        }
    }

    pub(super) fn send_resolution(&mut self, options: TreeOptions) {
        self.resolution_generation = self.resolution_generation.wrapping_add(1);
        let request = TreeResolutionRequest {
            generation: self.resolution_generation,
            format_requirements: self.format.requirements().with_metadata(),
            options,
        };
        if self.resolution_tx.send(request).is_ok() {
            self.resolution_applying = true;
        } else {
            self.invert_reverting = false;
            if let Some(target) = &self.invert_target {
                self.invert_prompt =
                    Some(format!("{target} (could not start dependency resolution)"));
            }
        }
    }
}
