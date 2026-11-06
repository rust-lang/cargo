#![allow(dead_code)]

use super::common;
use crate::prelude::*;

use std::sync::{Arc, mpsc};

use cargo::ops::cargo_tree::tui::{
    TuiTree,
    dependency::{
        Dependency, DependencyFeature, DependencyNode, DependencyTree, NodeId,
        PackageFormatMetadata, PackageFormatRequirements, ResolutionOptions,
    },
    draw_tui,
    metadata::MetadataTab,
    selector::{SelectorOption, SelectorState},
    state::{InputMode, TreeOptions, TuiChannels, TuiState},
    widget::{TreeCharset, TreeFormat, TreePrefix},
};
use cargo_test_support::project;
use common::{TestNode, TestNodeKind, build_tree};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Style};

const WIDTH: u16 = 80;
const HEIGHT: u16 = 24;

fn state_for_tree(tree: DependencyTree, manifest_path: Option<std::path::PathBuf>) -> TuiState {
    let (search, _) = mpsc::channel();
    let (resolution, _) = mpsc::channel();
    TuiState::new(
        Arc::new(TuiTree::new(tree, true)),
        TuiChannels { search, resolution },
        None,
        TreePrefix::default(),
        TreeCharset::Utf8,
        TreeFormat::parse("{p}").unwrap(),
        TreeOptions {
            manifest_path,
            resolution: ResolutionOptions::default(),
            no_dedupe: true,
        },
    )
}

fn sample_tree() -> DependencyTree {
    let nodes = [
        TestNode {
            name: "cargo-tree-tui",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "cargo",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "anyhow",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "termwiz",
            parent: Some(1),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];
    let mut tree = build_tree(&nodes);
    for node in &mut tree.nodes {
        if let DependencyNode::Crate(dependency) = node {
            dependency.version = match dependency.name.as_str() {
                "cargo-tree-tui" => "0.1.0",
                "cargo" => "0.95.0",
                "anyhow" => "1.0.104",
                "termwiz" => "0.23.3",
                _ => unreachable!(),
            }
            .to_owned();
        }
    }

    tree
}

fn state() -> TuiState {
    state_for_tree(sample_tree(), None)
}

fn feature_project() -> cargo_test_support::Project {
    project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["app", "codec"]
            resolver = "3"
            "#,
        )
        .file(
            "app/Cargo.toml",
            r#"
            [package]
            name = "feature-cli-app"
            version = "0.1.0"
            edition = "2024"

            [features]
            ui = ["feature-cli-codec/derive"]

            [dependencies]
            feature-cli-codec = { path = "../codec" }
            "#,
        )
        .file("app/src/lib.rs", "")
        .file(
            "codec/Cargo.toml",
            r#"
            [package]
            name = "feature-cli-codec"
            version = "0.1.0"
            edition = "2024"

            [features]
            default = ["std"]
            std = []
            derive = []
            "#,
        )
        .file("codec/src/lib.rs", "")
        .build()
}

fn press(state: &mut TuiState, key: char) {
    press_code(state, KeyCode::Char(key));
}

fn press_code(state: &mut TuiState, code: KeyCode) {
    state.handle_event(cargo::ops::cargo_tree::tui::state::Event::Key(
        KeyEvent::new(code, KeyModifiers::NONE),
    ));
}

fn render(state: &mut TuiState) -> String {
    render_at(state, WIDTH, HEIGHT)
}

fn render_at(state: &mut TuiState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw_tui(frame, state)).unwrap();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let canonical_manifest_dir = manifest_dir.canonicalize().unwrap();
    buffer_as_ansi(terminal.backend().buffer())
        .replace(manifest_dir.to_str().unwrap(), "$CARGO_MANIFEST_DIR")
        .replace(
            canonical_manifest_dir.to_str().unwrap(),
            "$CARGO_MANIFEST_DIR",
        )
}

fn buffer_as_ansi(buffer: &ratatui::buffer::Buffer) -> String {
    let mut output = String::new();
    let mut current_style = Style::default();
    for y in buffer.area.y..buffer.area.bottom() {
        for x in buffer.area.x..buffer.area.right() {
            let cell = buffer.cell((x, y)).unwrap();
            let style = cell.style();
            if style != current_style {
                output.push_str(&ansi_style(style));
                current_style = style;
            }
            output.push_str(cell.symbol());
        }
        output.push_str("\x1b[0m\n");
        current_style = Style::default();
    }
    output
}

fn ansi_style(style: Style) -> String {
    let mut codes = vec!["0".to_owned()];
    if let Some(fg) = style.fg {
        color_codes(fg, true, &mut codes);
    }
    if let Some(bg) = style.bg {
        color_codes(bg, false, &mut codes);
    }
    use ratatui::style::Modifier as M;
    for (modifier, code) in [
        (M::BOLD, "1"),
        (M::DIM, "2"),
        (M::ITALIC, "3"),
        (M::UNDERLINED, "4"),
        (M::SLOW_BLINK, "5"),
        (M::RAPID_BLINK, "6"),
        (M::REVERSED, "7"),
        (M::HIDDEN, "8"),
        (M::CROSSED_OUT, "9"),
    ] {
        if style
            .add_modifier
            .difference(style.sub_modifier)
            .contains(modifier)
        {
            codes.push(code.to_owned());
        }
    }
    format!("\x1b[{}m", codes.join(";"))
}

fn color_codes(color: ratatui::style::Color, foreground: bool, codes: &mut Vec<String>) {
    use ratatui::style::Color as C;
    let basic = match color {
        C::Black => Some(0),
        C::Red => Some(1),
        C::Green => Some(2),
        C::Yellow => Some(3),
        C::Blue => Some(4),
        C::Magenta => Some(5),
        C::Cyan => Some(6),
        C::Gray => Some(7),
        C::DarkGray => Some(8),
        C::LightRed => Some(9),
        C::LightGreen => Some(10),
        C::LightYellow => Some(11),
        C::LightBlue => Some(12),
        C::LightMagenta => Some(13),
        C::LightCyan => Some(14),
        C::White => Some(15),
        _ => None,
    };
    if let Some(index) = basic {
        let offset = if foreground { 30 } else { 40 };
        codes.push((offset + index + if index >= 8 { 52 } else { 0 }).to_string());
        return;
    }
    let base = if foreground { 38 } else { 48 };
    match color {
        C::Indexed(index) => codes.extend([base.to_string(), "5".to_owned(), index.to_string()]),
        C::Rgb(r, g, b) => codes.extend([
            base.to_string(),
            "2".to_owned(),
            r.to_string(),
            g.to_string(),
            b.to_string(),
        ]),
        _ => {}
    }
}

fn snapshot(name: &str, mut state: TuiState) {
    snapshot_at(name, &mut state, WIDTH, HEIGHT);
}

fn snapshot_at(name: &str, state: &mut TuiState, width: u16, height: u16) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testsuite/cargo_tree/tui/screens")
        .join(format!("{name}.term.svg"));
    snapbox::assert_data_eq!(
        render_at(state, width, height),
        snapbox::Data::read_from(&path, None)
    );
}

#[test]
fn main_tree() {
    snapshot("main-tree", state());
}

#[test]
fn ascii_tree() {
    let mut state = state();
    state.charset = TreeCharset::Ascii;
    snapshot("ascii-tree", state);
}

#[test]
fn depth_prefix_tree() {
    let mut state = state();
    state.prefix = TreePrefix::Depth;
    snapshot("depth-prefix-tree", state);
}

#[test]
fn no_prefix_tree() {
    let mut state = state();
    state.prefix = TreePrefix::None;
    snapshot("no-prefix-tree", state);
}

#[test]
fn collapsed_selected_branch() {
    let mut state = state();
    state
        .tree_widget_state
        .set_selected_node_id(&state.dependency_tree, NodeId(1));
    state.tree_widget_state.collapse(&state.dependency_tree);
    snapshot("collapsed-selected-branch", state);
}

#[test]
fn scrolled_tree_with_scrollbar() {
    let mut nodes = Vec::with_capacity(33);
    let children = (1..33).map(NodeId).collect::<Vec<_>>();
    nodes.push(DependencyNode::Crate(Dependency {
        name: "workspace".to_owned(),
        version: "".to_owned(),
        package_id: None,
        manifest_dir: None,
        is_proc_macro: false,
        format_metadata: None,
        children,
    }));
    for index in 0..32 {
        nodes.push(DependencyNode::Crate(Dependency {
            name: format!("dependency-{index:02}"),
            version: "1.0.0".to_owned(),
            package_id: None,
            manifest_dir: None,
            is_proc_macro: false,
            format_metadata: None,
            children: Vec::new(),
        }));
    }
    let tree = DependencyTree {
        workspace_name: "workspace".to_owned(),
        parents: (0..nodes.len())
            .map(|index| {
                if index == 0 {
                    Vec::new()
                } else {
                    vec![NodeId(0)]
                }
            })
            .collect(),
        dependency_usages: Default::default(),
        nodes,
        roots: vec![NodeId(0)],
    };
    let mut state = state_for_tree(tree, None);
    state
        .tree_widget_state
        .set_selected_node_id(&state.dependency_tree, NodeId(28));
    snapshot("scrolled-tree-scrollbar", state);
}

#[test]
fn feature_edge_tree() {
    let nodes = [TestNode {
        name: "app",
        parent: None,
        children: &[],
        kind: TestNodeKind::Crate,
    }];
    let mut tree = build_tree(&nodes);
    tree.nodes.push(DependencyNode::Feature(DependencyFeature {
        package_name: "app".to_owned(),
        name: "default".to_owned(),
        command_line: false,
        children: vec![],
    }));
    if let DependencyNode::Crate(dependency) = &mut tree.nodes[0] {
        dependency.children.push(NodeId(1));
    }
    tree.parents.push(vec![NodeId(0)]);
    snapshot("feature-edge-tree", state_for_tree(tree, None));
}

#[test]
fn search_prompt() {
    let mut state = state();
    state.input_mode = InputMode::Search;
    state.search_query = "serde".to_owned();
    snapshot("search-prompt", state);
}

#[test]
fn search_results() {
    let mut state = state();
    state.input_mode = InputMode::SearchResults;
    state.search_query = "termwiz".to_owned();
    state
        .tree_widget_state
        .set_search_query(&state.dependency_tree, &state.search_query);
    snapshot("search-results", state);
}

#[test]
fn help_popup() {
    let mut state = state();
    state.show_help = true;
    snapshot("help-popup", state);
}

#[test]
fn edge_selector() {
    let mut state = state();
    state.selector = Some(SelectorState::new(
        "Dependency edge kinds",
        vec![
            SelectorOption::new("normal", "Normal dependencies", true),
            SelectorOption::new("build", "Build dependencies", true),
            SelectorOption::new("dev", "Development dependencies", false),
            SelectorOption::new("features", "Feature edges", false),
        ],
    ));
    state.input_mode = InputMode::Selector;
    snapshot("edge-selector", state);
}

#[test]
fn target_selector_with_filter() {
    let mut state = state();
    state.selector = Some(
        SelectorState::new(
            "Target triples",
            vec![
                SelectorOption::new("default", "Default target", true).exclusive(),
                SelectorOption::new("all", "All targets", false).exclusive(),
                SelectorOption::new(
                    "x86_64-unknown-linux-gnu",
                    "x86_64-unknown-linux-gnu",
                    false,
                ),
            ],
        )
        .filterable(),
    );
    state.selector.as_mut().unwrap().push_filter_char('x');
    state.input_mode = InputMode::Selector;
    snapshot("target-selector-filter", state);
}

#[test]
fn target_selector_all_targets_checked() {
    let mut state = state();
    state.selector = Some(
        SelectorState::new(
            "Target triples",
            vec![
                SelectorOption::new("default", "Default target", false).exclusive(),
                SelectorOption::new("all", "all", true).exclusive(),
                SelectorOption::new(
                    "x86_64-unknown-linux-gnu",
                    "x86_64-unknown-linux-gnu",
                    false,
                ),
                SelectorOption::new(
                    "aarch64-unknown-linux-gnu",
                    "aarch64-unknown-linux-gnu",
                    false,
                ),
            ],
        )
        .filterable(),
    );
    state.input_mode = InputMode::Selector;
    snapshot("target-selector-all", state);
}

#[test]
fn selector_empty_filter_results() {
    let mut state = state();
    let mut selector = SelectorState::new(
        "Target triples",
        vec![
            SelectorOption::new("default", "Default target", true),
            SelectorOption::new("all", "all", false),
        ],
    )
    .filterable();
    selector.push_filter_char('z');
    state.selector = Some(selector);
    state.input_mode = InputMode::Selector;
    snapshot("selector-empty-filter", state);
}

#[test]
fn selector_loading_error() {
    let mut state = state();
    let mut selector = SelectorState::new(
        "Features",
        vec![SelectorOption::new("__default", "Default features", true)],
    )
    .filterable();
    selector.message = Some("failed to load workspace features".to_owned());
    state.selector = Some(selector);
    state.input_mode = InputMode::Selector;
    snapshot("selector-loading-error", state);
}

#[test]
fn selector_applying() {
    let mut state = state();
    let mut selector = SelectorState::new(
        "Features",
        vec![SelectorOption::new("serde", "serde", true)],
    )
    .filterable();
    selector.applying = true;
    state.selector = Some(selector);
    state.input_mode = InputMode::Selector;
    snapshot("selector-applying", state);
}

#[test]
fn selector_scrollbar() {
    let mut state = state();
    let options = (0..24)
        .map(|index| {
            SelectorOption::new(
                format!("target-{index}"),
                format!("x86_64-unknown-linux-gnu-{index}"),
                index == 18,
            )
        })
        .collect();
    let mut selector = SelectorState::new("Target triples", options).filterable();
    selector.selected = 18;
    state.selector = Some(selector);
    state.input_mode = InputMode::Selector;
    snapshot("selector-scrollbar", state);
}

#[test]
fn feature_selector() {
    let mut state = state();
    state.selector = Some(
        SelectorState::new(
            "Features",
            vec![
                SelectorOption::new("__default", "Default features", true).exclusive(),
                SelectorOption::new("__all", "All features", false).exclusive(),
                SelectorOption::new("derive", "derive", true),
                SelectorOption::new("serde", "serde", false),
            ],
        )
        .filterable(),
    );
    state.input_mode = InputMode::Selector;
    snapshot("feature-selector", state);
}

#[test]
fn package_selector_package_tab_with_filter() {
    let mut state = state();
    let mut selector = SelectorState::new(
        "Package selection",
        vec![
            SelectorOption::new("workspace", "All workspace packages", true).in_tab(0),
            SelectorOption::new("+cargo-tree-tui", "cargo-tree-tui", false).in_tab(0),
            SelectorOption::new("+cargo", "cargo", true).in_tab(0),
            SelectorOption::new("-cargo", "cargo", false).in_tab(1),
        ],
    )
    .filterable()
    .tabs(vec!["Package".to_owned(), "Exclude".to_owned()]);
    selector.push_filter_char('c');
    selector.push_filter_char('a');
    selector.push_filter_char('r');
    selector.push_filter_char('g');
    selector.push_filter_char('o');
    state.selector = Some(selector);
    state.input_mode = InputMode::Selector;
    snapshot("package-selector-package-filter", state);
}

#[test]
fn package_selector_exclude_tab() {
    let mut state = state();
    let mut selector = SelectorState::new(
        "Package selection",
        vec![
            SelectorOption::new("workspace", "All workspace packages", true).in_tab(0),
            SelectorOption::new("+cargo-tree-tui", "cargo-tree-tui", false).in_tab(0),
            SelectorOption::new("-cargo-tree-tui", "cargo-tree-tui", true).in_tab(1),
        ],
    )
    .filterable()
    .tabs(vec!["Package".to_owned(), "Exclude".to_owned()]);
    selector.next_tab();
    state.selector = Some(selector);
    state.input_mode = InputMode::Selector;
    snapshot("package-selector-exclude", state);
}

#[test]
fn selector_small_terminal() {
    let mut state = state();
    state.selector = Some(
        SelectorState::new(
            "Target triples",
            vec![
                SelectorOption::new("default", "Default target", true).exclusive(),
                SelectorOption::new("all", "All targets", false).exclusive(),
                SelectorOption::new(
                    "x86_64-unknown-linux-gnu",
                    "x86_64-unknown-linux-gnu",
                    false,
                ),
            ],
        )
        .filterable(),
    );
    state.input_mode = InputMode::Selector;
    snapshot_at("selector-small-terminal", &mut state, 38, 10);
}

#[test]
fn format_selector() {
    let mut state = state();
    state.selector = Some(
        SelectorState::new(
            "Package format",
            vec![
                SelectorOption::new("version", "Version", true),
                SelectorOption::new("license", "License", false),
                SelectorOption::new("repository", "Repository", false),
            ],
        )
        .editor("{p} v{v}"),
    );
    state.input_mode = InputMode::Selector;
    snapshot("format-selector", state);
}

#[cargo_test]
fn selectors_opened_by_their_key_bindings() {
    let project = feature_project();
    let manifest = project.root().join("app/Cargo.toml");

    let mut state = state();
    press(&mut state, 'e');
    snapshot("keybinding-edge-selector", state);

    let mut state = state_for_tree(sample_tree(), None);
    press(&mut state, 'e');
    press_code(&mut state, KeyCode::Char(' '));
    snapshot("keybinding-edge-selector-toggled", state);

    let mut state = state_for_tree(sample_tree(), Some(manifest.clone()));
    state.tree_options.resolution.targets = vec!["x86_64-unknown-linux-gnu".to_owned()];
    press(&mut state, 't');
    for ch in "x86_64-unknown-linux-gnu".chars() {
        press(&mut state, ch);
    }
    snapshot("keybinding-target-selector-filtered", state);

    let mut state = state_for_tree(sample_tree(), Some(manifest.clone()));
    state.tree_options.resolution.features = vec!["ui".to_owned()];
    state.tree_options.resolution.no_default_features = true;
    press(&mut state, 'f');
    snapshot("keybinding-feature-selector", state);

    let mut state = state_for_tree(sample_tree(), Some(manifest.clone()));
    press(&mut state, 'p');
    snapshot("keybinding-package-selector", state);
    let mut state = state_for_tree(sample_tree(), Some(manifest));
    press(&mut state, 'p');
    press_code(&mut state, KeyCode::Tab);
    snapshot("keybinding-exclude-selector", state);

    let mut state = state_for_tree(sample_tree(), None);
    press(&mut state, 'M');
    snapshot("keybinding-format-selector", state);
}

#[test]
fn metadata_package_tab() {
    let mut state = state();
    state.show_metadata = true;
    state.tree_widget_state.select_next(&state.dependency_tree);
    snapshot("metadata-package", state);
}

fn rich_metadata_state() -> TuiState {
    let mut tree = sample_tree();
    let source =
        cargo::workspace::SourceId::for_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    if let Some(DependencyNode::Crate(dependency)) = tree.nodes.get_mut(1) {
        dependency.package_id =
            Some(cargo::workspace::PackageId::try_new("cargo", "0.95.0", source).unwrap());
        dependency.format_metadata = Some(Arc::new(PackageFormatMetadata {
            description: Some("A deliberately long crate description used to exercise wrapping and scrolling inside the dependency metadata panel.".to_owned()),
            edition: Some("2024".to_owned()),
            rust_version: Some("1.85".to_owned()),
            targets: Some("library, binary, integration test".to_owned()),
            license: Some("MIT OR Apache-2.0".to_owned()),
            repository: Some("https://github.com/example/cargo".to_owned()),
            homepage: Some("https://example.com/cargo".to_owned()),
            documentation: Some("https://docs.rs/cargo".to_owned()),
            authors: Some("Cargo contributors".to_owned()),
            keywords: Some("package, resolver".to_owned()),
            categories: Some("development-tools".to_owned()),
            features: Some("default, http".to_owned()),
            library: Some("cargo".to_owned()),
            source: Some("crates.io".to_owned()),
        }));
    }
    let mut state = state_for_tree(tree, None);
    state.show_metadata = true;
    state
        .tree_widget_state
        .set_selected_node_id(&state.dependency_tree, NodeId(1));
    state
}

#[test]
fn metadata_with_all_optional_fields() {
    let mut state = rich_metadata_state();
    snapshot_at("metadata-package-rich", &mut state, 100, 32);
}

#[test]
fn metadata_with_all_optional_fields_and_scroll() {
    let mut state = rich_metadata_state();
    state.metadata_popup.scroll = 5;
    snapshot_at("metadata-package-scrolled", &mut state, 80, 16);
}

#[test]
fn metadata_duplicate_versions() {
    let source =
        cargo::workspace::SourceId::for_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let mut tree = build_tree(&[
        TestNode {
            name: "workspace",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "serde",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "serde",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ]);
    for (id, version) in [(1, "1.0.0"), (2, "2.0.0")] {
        if let DependencyNode::Crate(dependency) = &mut tree.nodes[id] {
            dependency.version = version.to_owned();
            dependency.package_id =
                Some(cargo::workspace::PackageId::try_new("serde", version, source).unwrap());
            dependency.format_metadata = Some(Arc::new(PackageFormatMetadata {
                description: Some("A serialization framework".to_owned()),
                edition: Some("2021".to_owned()),
                rust_version: Some("1.56".to_owned()),
                license: Some("MIT OR Apache-2.0".to_owned()),
                repository: Some("https://github.com/serde-rs/serde".to_owned()),
                features: Some("default, derive".to_owned()),
                source: Some("crates.io".to_owned()),
                ..PackageFormatMetadata::default()
            }));
        }
    }
    let mut state = state_for_tree(tree, None);
    state.show_metadata = true;
    state
        .tree_widget_state
        .set_selected_node_id(&state.dependency_tree, NodeId(1));
    snapshot_at("metadata-duplicate-versions", &mut state, 100, 24);
}

#[test]
fn metadata_usage_tab_without_declarations() {
    let mut state = state();
    state.show_metadata = true;
    state.metadata_popup.tab = MetadataTab::Usage;
    state.tree_widget_state.select_next(&state.dependency_tree);
    snapshot("metadata-usage-empty", state);
}

#[cargo_test]
fn metadata_usage_with_declaration() {
    let project = feature_project();
    let manifest = project.root().join("app/Cargo.toml");
    let mut tree = DependencyTree::load_with_options_and_format(
        Some(manifest.clone()),
        &ResolutionOptions::default(),
        PackageFormatRequirements::default().with_metadata(),
    )
    .unwrap();
    let selected = tree
        .crate_nodes()
        .find(|id| {
            tree.node(*id)
                .is_some_and(|node| node.display_name() == "feature-cli-codec")
        })
        .expect("test workspace contains feature-cli-codec");
    let package_id = tree
        .node(selected)
        .and_then(DependencyNode::as_dependency)
        .and_then(|dependency| dependency.package_id)
        .expect("test workspace dependency has a Cargo package ID");
    assert!(tree.dependency_usages.contains_key(&package_id));
    assert!(!tree.dependency_usages[&package_id].is_empty());
    for usage in &tree.dependency_usages[&package_id] {
        assert!(tree.crate_nodes().any(|id| {
            tree.node(id)
                .and_then(DependencyNode::as_dependency)
                .is_some_and(|dependency| dependency.package_id == Some(usage.parent))
        }));
    }
    // Keep the narrow usage-tree labels independent of the checkout path.
    for node in &mut tree.nodes {
        if let DependencyNode::Crate(dependency) = node
            && dependency.manifest_dir.is_some()
        {
            dependency.manifest_dir = Some("workspace".to_owned());
            if let Some(metadata) = &mut dependency.format_metadata {
                Arc::make_mut(metadata).source = Some("workspace".to_owned());
            }
        }
    }
    let mut state = state_for_tree(tree, Some(manifest));
    state.show_metadata = true;
    state.metadata_popup.tab = MetadataTab::Usage;
    state
        .tree_widget_state
        .set_selected_node_id(&state.dependency_tree, selected);
    let rendered = render_at(&mut state, 120, 36);
    let selected_dependency =
        TuiState::selected_dependency(&state.dependency_tree, &state.tree_widget_state).unwrap();
    assert_eq!(selected_dependency.name, "feature-cli-codec");
    assert!(
        !state.dependency_tree.dependency_usages[&selected_dependency.package_id.unwrap()]
            .is_empty()
    );
    assert!(rendered.contains("Dependency declaration"));
    snapshot_at("metadata-usage-declaration", &mut state, 120, 36);
}
