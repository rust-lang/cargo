use cargo::ops::cargo_tree::tui::TuiTree;
use cargo::ops::cargo_tree::tui::dependency::{
    Dependency, DependencyGroup, DependencyNode, DependencyTree, NodeId,
};
use cargo::ops::cargo_tree::tui::widget::render::RenderContext;
use cargo::ops::cargo_tree::tui::widget::{
    TreePrefix, TreeWidget, TreeWidgetState, TreeWidgetStyle,
};
use cargo::workspace::dependency::DepKind;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

pub enum TestNodeKind {
    Crate,
    Group(DepKind),
}

pub struct TestNode {
    pub name: &'static str,
    pub parent: Option<usize>,
    pub children: &'static [usize],
    pub kind: TestNodeKind,
}

pub fn build_tree(nodes: &[TestNode]) -> DependencyTree {
    let mut arena = Vec::with_capacity(nodes.len());
    for node in nodes {
        let children = node.children.iter().copied().map(NodeId).collect();
        let node = match node.kind {
            TestNodeKind::Crate => DependencyNode::Crate(Dependency {
                name: node.name.into(),
                version: "".into(),
                package_id: None,
                manifest_dir: None,
                is_proc_macro: false,
                format_metadata: None,
                children,
            }),
            TestNodeKind::Group(kind) => DependencyNode::Group(DependencyGroup { kind, children }),
        };
        arena.push(node);
    }

    let roots = nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| node.parent.is_none().then_some(NodeId(idx)))
        .collect();

    // Build reverse parent map from children lists.
    let mut parents = vec![Vec::new(); arena.len()];
    for (idx, node) in arena.iter().enumerate() {
        let parent_id = NodeId(idx);
        for &child_id in node.children() {
            parents[child_id.0].push(parent_id);
        }
    }

    DependencyTree {
        workspace_name: "workspace".into(),
        parents,
        dependency_usages: Default::default(),
        nodes: arena,
        roots,
    }
}

pub fn render_tree_context(tree: &DependencyTree) -> String {
    let tree = TuiTree::new(tree.clone(), true);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);

    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    let style = TreeWidgetStyle::default();
    let mut context =
        RenderContext::new(&tree, &mut state, &style, None, TreePrefix::default(), None);
    let output = context.render(area);

    output
        .lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn render_tree_widget(
    tree: &DependencyTree,
    state: &mut TreeWidgetState,
    area: Rect,
) -> String {
    let tree = TuiTree::new(tree.clone(), true);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| {
            let frame_area = frame.area();
            TreeWidget::new(&tree).render(frame_area, frame.buffer_mut(), state);
        })
        .unwrap();
    terminal
        .backend()
        .to_string()
        .lines()
        .map(|s| s.trim_start_matches('"').trim_end_matches('"').trim_end())
        .collect::<Vec<&str>>()
        .join("\n")
}
