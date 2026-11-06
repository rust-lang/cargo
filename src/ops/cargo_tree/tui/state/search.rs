//! Run searches and apply results to the visible tree.

use super::*;

impl TuiState {
    pub(super) fn handle_search_result(&mut self, search_result: SearchResult) {
        if search_result.generation != self.search_generation
            || search_result.query != self.search_query
        {
            return;
        }

        self.search_running = false;
        self.tree_widget_state
            .apply_search_state(&self.dependency_tree, search_result.search_state);
        self.metadata_popup.scroll = 0;
    }

    pub(super) fn request_search(&mut self) {
        self.search_generation += 1;
        let request = SearchRequest {
            generation: self.search_generation,
            query: self.search_query.clone(),
            dependency_tree: Arc::clone(&self.dependency_tree),
        };

        if request.query.is_empty() {
            self.search_running = false;
            self.tree_widget_state.clear_search();
            return;
        }

        self.search_running = true;
        let _ = self.search_tx.send(request);
    }

    pub(super) fn clear_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.search_generation += 1;
        self.search_query.clear();
        self.search_running = false;
        self.tree_widget_state.clear_search();
    }

    pub(super) fn invert_selected_package(&mut self) {
        let Some(node_id) = self.tree_widget_state.selected_node_id() else {
            return;
        };
        let Some(DependencyNode::Crate(dependency)) = self.dependency_tree.node(node_id) else {
            return;
        };

        let Some(package_id) = dependency.package_id else {
            return;
        };
        let package_spec = package_id.to_spec().to_string();
        if self.invert_restore.is_none() {
            self.invert_restore = Some(self.tree_options.resolution.invert.clone());
        }
        let target = format!("{} v{}", dependency.name, dependency.version);
        self.invert_target = Some(target.clone());
        self.invert_prompt = Some(target.clone());
        self.invert_reverting = false;

        let mut options = self.tree_options.clone();
        options.resolution.invert = Some(vec![package_spec]);
        self.send_resolution(options);
    }

    pub(super) fn restore_inverted_tree(&mut self) {
        let Some(original_invert) = self.invert_restore.clone() else {
            return;
        };
        let mut options = self.tree_options.clone();
        options.resolution.invert = original_invert;
        self.invert_reverting = true;
        self.send_resolution(options);
    }
}
