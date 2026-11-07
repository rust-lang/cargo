use super::common;

use cargo::ops::cargo_tree::tui::dependency::NodeId;
use cargo::ops::cargo_tree::tui::widget::TreeWidgetState;
use cargo::workspace::dependency::DepKind;
use common::{TestNode, TestNodeKind, build_tree, render_tree_context, render_tree_widget};
use ratatui::layout::Rect;

#[test]
fn basic() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(1),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let expected = r#"
root
├──▾ a
│  └──• c
└──• b
"#;

    let tree = build_tree(&nodes);
    let tree_str = render_tree_context(&tree);
    assert_eq!(expected.trim(), tree_str.trim());
}

#[test]
fn root_dev_dependencies_header() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "dev",
            parent: Some(0),
            children: &[2],
            kind: TestNodeKind::Group(DepKind::Development),
        },
        TestNode {
            name: "a",
            parent: Some(1),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let expected = r#"
root
[dev-dependencies]
└──• a
"#;

    let tree = build_tree(&nodes);
    assert_eq!(expected.trim(), render_tree_context(&tree).trim());
}

#[test]
fn root_normal_deps_then_dev_header() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "dev",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Group(DepKind::Development),
        },
        TestNode {
            name: "b",
            parent: Some(2),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let expected = r#"
root
└──• a
[dev-dependencies]
└──• b
"#;

    let tree = build_tree(&nodes);
    assert_eq!(expected.trim(), render_tree_context(&tree).trim());
}

#[test]
fn nested_dev_dependencies_header() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "dev",
            parent: Some(1),
            children: &[3],
            kind: TestNodeKind::Group(DepKind::Development),
        },
        TestNode {
            name: "b",
            parent: Some(2),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let expected = r#"
root
└──▾ a
   [dev-dependencies]
   └──• b
"#;

    let tree = build_tree(&nodes);
    assert_eq!(expected.trim(), render_tree_context(&tree).trim());
}

#[test]
fn nested_header_preserves_guides() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "x",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "dev",
            parent: Some(2),
            children: &[4],
            kind: TestNodeKind::Group(DepKind::Development),
        },
        TestNode {
            name: "b",
            parent: Some(3),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let expected = r#"
root
├──• x
└──▾ a
   [dev-dependencies]
   └──• b
"#;

    let tree = build_tree(&nodes);
    assert_eq!(expected.trim(), render_tree_context(&tree).trim());
}

#[test]
fn nested_group_header_with_following_sibling() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 4],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "x",
            parent: Some(0),
            children: &[2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "dev",
            parent: Some(1),
            children: &[3],
            kind: TestNodeKind::Group(DepKind::Development),
        },
        TestNode {
            name: "b",
            parent: Some(2),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let expected = r#"
root
├──▾ x
│  [dev-dependencies]
│  └──• b
└──• a
"#;

    let tree = build_tree(&nodes);
    assert_eq!(expected.trim(), render_tree_context(&tree).trim());
}

#[test]
fn breadcrumb_when_scrolled() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(1),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(2),
            children: &[4],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "d",
            parent: Some(3),
            children: &[5],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "e",
            parent: Some(4),
            children: &[6],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "f",
            parent: Some(5),
            children: &[7],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "g",
            parent: Some(6),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);
    state.set_selected_node_id(&tree, NodeId(7));

    let area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 5,
    };

    let expected = r#"
root
└──▾ a
   └──▾ b
      └──▾ c
root → a → b → c → d → e → f → g
"#;

    let output = render_tree_widget(&tree, &mut state, area);
    assert_eq!(expected.trim(), output.trim());

    let area = Rect {
        x: 0,
        y: 0,
        width: 55,
        height: 5,
    };

    state.set_selected_node_id(&tree, NodeId(7));

    let expected = r#"
root
└──▾ a
   └──▾ b
      └──▾ c
root → a → b → … → g
"#;

    let output = render_tree_widget(&tree, &mut state, area);
    assert_eq!(expected.trim(), output.trim());
}

#[test]
fn context_bar_when_scrolled() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(1),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(2),
            children: &[4],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "d",
            parent: Some(3),
            children: &[5],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "e",
            parent: Some(4),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);
    state.set_selected_node_id(&tree, NodeId(5));

    let area = Rect {
        x: 0,
        y: 0,
        width: 55,
        height: 6,
    };

    let expected = r#"
root
   └──▾ b
      └──▾ c
         └──▾ d
            └──• e
root → a → b → … → e
"#;

    let output = render_tree_widget(&tree, &mut state, area);
    assert_eq!(expected.trim(), output.trim());
}

// ── Virtual flattening / windowed materialization tests ─────────────

/// A DAG with shared subtrees: root -> {a, b}, a -> c, b -> c, c -> d.
/// With expand_all, `c`'s subtree is counted under both `a` and `b`.
#[test]
fn dag_shared_subtree_expand_all() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(1),
            children: &[4],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "d",
            parent: Some(3),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);

    // root(1) + a(1) + c(1) + d(1) + b(1) + c(1) + d(1) = 7
    assert_eq!(state.total_lines(&tree), 7);
}

/// Expand-all on a DAG renders shared subtrees under each parent.
#[test]
fn dag_shared_subtree_renders() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(1),
            children: &[4],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "d",
            parent: Some(3),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let tree_str = render_tree_context(&tree);
    let expected = r#"
root
├──▾ a
│  └──▾ c
│     └──• d
└──▾ b
   └──▾ c
      └──• d
"#;

    assert_eq!(expected.trim(), tree_str.trim());
}

/// Navigation through a DAG: select_next walks all virtual positions in DFS order.
#[test]
fn dag_navigation_select_next() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(1),
            children: &[4],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "d",
            parent: Some(3),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);

    let mut visited = Vec::new();
    for _ in 0..7 {
        state.ensure_visible_nodes(&tree);
        let node_id = state.selected_node_id().unwrap();
        visited.push(tree.node(node_id).unwrap().display_name().to_string());
        state.select_next(&tree);
    }

    assert_eq!(visited, vec!["root", "a", "c", "d", "b", "c", "d"]);
}

/// Collapse and expand update total_lines correctly.
#[test]
fn collapse_expand_virtual_pos() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(1),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);
    assert_eq!(state.total_lines(&tree), 4);

    // Select "a" and collapse it.
    state.select_next(&tree);
    state.ensure_visible_nodes(&tree);
    assert_eq!(state.selected_node_id().map(|id| id.0), Some(1));
    state.collapse(&tree);
    assert_eq!(state.total_lines(&tree), 3); // root, a(collapsed), b

    // Expand again.
    state.expand(&tree);
    assert_eq!(state.total_lines(&tree), 4);
}

/// Large DAG with shared subtrees doesn't OOM or panic.
#[test]
fn large_dag_no_oom() {
    use cargo::ops::cargo_tree::tui::dependency::{Dependency, DependencyNode, DependencyTree};

    // root -> a0..a9 (shared b children), each bi -> c0..c9 (shared)
    let mut arena = Vec::new();

    let root_children: Vec<NodeId> = (1..=10).map(NodeId).collect();
    arena.push(DependencyNode::Crate(Dependency {
        name: "root".into(),
        version: "0.1.0".into(),
        package_id: None,
        manifest_dir: None,
        is_proc_macro: false,
        format_metadata: None,
        children: root_children,
    }));

    let b_children: Vec<NodeId> = (11..=20).map(NodeId).collect();
    for i in 0..10 {
        arena.push(DependencyNode::Crate(Dependency {
            name: format!("a{i}"),
            version: "0.1.0".into(),
            package_id: None,
            manifest_dir: None,
            is_proc_macro: false,
            format_metadata: None,
            children: b_children.clone(),
        }));
    }

    let c_children: Vec<NodeId> = (21..=30).map(NodeId).collect();
    for i in 0..10 {
        arena.push(DependencyNode::Crate(Dependency {
            name: format!("b{i}"),
            version: "0.1.0".into(),
            package_id: None,
            manifest_dir: None,
            is_proc_macro: false,
            format_metadata: None,
            children: c_children.clone(),
        }));
    }

    for i in 0..10 {
        arena.push(DependencyNode::Crate(Dependency {
            name: format!("c{i}"),
            version: "0.1.0".into(),
            package_id: None,
            manifest_dir: None,
            is_proc_macro: false,
            format_metadata: None,
            children: Vec::new(),
        }));
    }

    let mut parents = vec![Vec::new(); arena.len()];
    for (idx, node) in arena.iter().enumerate() {
        for &child in node.children() {
            parents[child.0].push(NodeId(idx));
        }
    }

    let tree = DependencyTree {
        workspace_name: "dag-test".into(),
        parents,
        dependency_usages: Default::default(),
        nodes: arena,
        roots: vec![NodeId(0)],
    };

    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);

    // 1 + 10*(1 + 10*(1 + 10)) = 1 + 10*111 = 1111
    assert_eq!(state.total_lines(&tree), 1111);

    for _ in 0..100 {
        state.select_next(&tree);
    }
    state.ensure_visible_nodes(&tree);
    assert!(state.selected_node_id().is_some());
}

/// set_selected_node_id locates a node by its first virtual position.
#[test]
fn set_selected_node_id_in_dag() {
    let nodes = [
        TestNode {
            name: "root",
            parent: None,
            children: &[1, 2],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "a",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[3],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "c",
            parent: Some(1),
            children: &[],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);

    state.set_selected_node_id(&tree, NodeId(2));
    state.ensure_visible_nodes(&tree);
    assert_eq!(state.selected_node_id().map(|id| id.0), Some(2));

    state.set_selected_node_id(&tree, NodeId(3));
    state.ensure_visible_nodes(&tree);
    assert_eq!(state.selected_node_id().map(|id| id.0), Some(3));
}

/// A cyclic dep graph (a -> b -> a) must yield a finite, terminating
/// view at the widget layer. Cargo permits cycles via dev-dependencies,
/// so the resolve graph fed into [`DependencyTree`] can legitimately
/// contain them. Without cycle breaking in `compute_subtree_sizes`,
/// `total_lines` would diverge; without bounding in materialization,
/// `expand_all` + render would loop forever.
#[test]
fn cyclic_tree_view_is_finite() {
    let nodes = [
        TestNode {
            name: "a",
            parent: None,
            children: &[1],
            kind: TestNodeKind::Crate,
        },
        TestNode {
            name: "b",
            parent: Some(0),
            children: &[0],
            kind: TestNodeKind::Crate,
        },
    ];

    let tree = build_tree(&nodes);
    let mut state = TreeWidgetState::default();
    state.expand_all(&tree);

    // The cycle breaker in `compute_subtree_sizes` treats the back-edge
    // as a leaf, so the visible tree unrolls to exactly:
    //   a            (size 1 + size(b) = 3)
    //   └─ b         (size 1 + size(a as leaf) = 2)
    //      └─ a      (cycle break, counted as 1)
    assert_eq!(state.total_lines(&tree), 3);

    // Render the materialized window: the cyclic tree must unroll once,
    // then stop on the back-edge.
    let area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    let rendered = render_tree_widget(&tree, &mut state, area);
    let tree_rows: Vec<&str> = rendered.lines().take(3).collect();
    assert_eq!(
        tree_rows,
        vec!["a", "└──▾ b", "   └──▾ a"],
        "full render:\n{rendered}"
    );
}
