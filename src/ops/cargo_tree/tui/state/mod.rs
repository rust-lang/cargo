//! Interactive tree state and event handling.

use std::path::PathBuf;
use std::{
    process::{Command, Stdio},
    sync::{Arc, OnceLock, mpsc::Sender},
};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::TuiTree;
use super::selector::{SelectorOption, SelectorState};
use crate::ops::cargo_tree::tui::dependency::{
    Dependency, PackageFormatRequirements, ResolutionOptions,
};
use crate::ops::cargo_tree::tui::dependency::{
    DependencyNode, workspace_features, workspace_packages,
};
use crate::workspace::dependency::DepKind;

use super::metadata::MetadataPopupState;
use super::widget::{SearchState, TreeCharset, TreeFormat, TreePrefix, TreeWidgetState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    SearchResults,
    Selector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorKind {
    EdgeKinds,
    Targets,
    Features,
    Packages,
    Format,
}

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    SearchResult(SearchResult),
    TreeResolved(TreeResolutionResult),
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub generation: u64,
    pub query: String,
    pub dependency_tree: Arc<TuiTree>,
}

#[derive(Debug)]
pub struct SearchResult {
    pub generation: u64,
    pub query: String,
    pub search_state: SearchState,
}

#[derive(Debug, Clone)]
pub struct TreeResolutionRequest {
    pub generation: u64,
    pub options: TreeOptions,
    pub format_requirements: PackageFormatRequirements,
}

#[derive(Debug)]
pub struct TreeResolutionResult {
    pub generation: u64,
    pub options: TreeOptions,
    pub result: Result<Arc<TuiTree>, String>,
}

/// CLI options used to resolve and prepare the tree shown in the TUI.
#[derive(Debug, Clone)]
pub struct TreeOptions {
    pub manifest_path: Option<PathBuf>,
    pub resolution: ResolutionOptions,
    pub no_dedupe: bool,
}

pub struct TuiChannels {
    pub search: Sender<SearchRequest>,
    pub resolution: Sender<TreeResolutionRequest>,
}

#[derive(Debug)]
pub struct TuiState {
    pub running: bool,
    pub dependency_tree: Arc<TuiTree>,
    pub tree_options: TreeOptions,
    pub selector: Option<SelectorState>,
    selector_kind: Option<SelectorKind>,
    feature_choices: Option<Vec<String>>,
    package_choices: Option<Vec<String>>,
    pub prefix: TreePrefix,
    pub charset: TreeCharset,
    pub format: TreeFormat,
    pending_format: Option<TreeFormat>,
    pub tree_widget_state: TreeWidgetState,
    pub show_help: bool,
    pub show_metadata: bool,
    pub metadata_popup: MetadataPopupState,
    pub input_mode: InputMode,
    pub search_query: String,
    pub search_running: bool,
    invert_restore: Option<Option<Vec<String>>>,
    invert_target: Option<String>,
    pub(crate) invert_prompt: Option<String>,
    invert_reverting: bool,
    spinner_frame: usize,
    search_generation: u64,
    search_tx: Sender<SearchRequest>,
    resolution_generation: u64,
    resolution_applying: bool,
    resolution_tx: Sender<TreeResolutionRequest>,
}

impl TuiState {
    pub fn new(
        dependency_tree: Arc<TuiTree>,
        channels: TuiChannels,
        depth: Option<usize>,
        prefix: TreePrefix,
        charset: TreeCharset,
        format: TreeFormat,
        tree_options: TreeOptions,
    ) -> Self {
        let mut tree_widget_state = TreeWidgetState::default();
        if let Some(depth) = depth {
            tree_widget_state.open_to_dependency_depth(&dependency_tree, depth);
        } else {
            tree_widget_state.expand_all(&dependency_tree);
        }
        TuiState {
            running: true,
            dependency_tree,
            tree_options,
            selector: None,
            selector_kind: None,
            feature_choices: None,
            package_choices: None,
            prefix,
            charset,
            format,
            pending_format: None,
            tree_widget_state,
            show_help: false,
            show_metadata: false,
            metadata_popup: MetadataPopupState::default(),
            input_mode: InputMode::Normal,
            search_query: String::new(),
            search_running: false,
            invert_restore: None,
            invert_target: None,
            invert_prompt: None,
            invert_reverting: false,
            spinner_frame: 0,
            search_generation: 0,
            search_tx: channels.search,
            resolution_generation: 0,
            resolution_applying: false,
            resolution_tx: channels.resolution,
        }
    }

    pub fn resolution_pending(&self) -> bool {
        self.resolution_applying
            || self
                .selector
                .as_ref()
                .is_some_and(|selector| selector.applying)
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event),
            Event::SearchResult(search_result) => self.handle_search_result(search_result),
            Event::TreeResolved(result) => self.handle_tree_resolved(result),
        }
    }

    pub fn advance_spinner(&mut self) {
        if self.search_running {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    pub fn search_prompt_symbol(&self) -> char {
        const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
        if self.search_running {
            FRAMES[self.spinner_frame % FRAMES.len()]
        } else {
            '/'
        }
    }

    pub fn selected_dependency<'a>(
        tree: &'a TuiTree,
        tree_widget_state: &TreeWidgetState,
    ) -> Option<&'a Dependency> {
        let mut node_id = tree_widget_state.selected_node_id()?;
        loop {
            let node = tree.node(node_id)?;
            if let Some(dependency) = node.as_dependency() {
                return Some(dependency);
            }
            node_id = *tree.parents.get(node_id.0)?.first()?;
        }
    }
}

mod events;
mod links;
mod resolution;
mod search;
mod selectors;
