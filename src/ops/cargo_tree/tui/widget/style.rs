use crate::util::style::{DEP_BUILD, DEP_FEATURE, NOP, PLACEHOLDER, WARN};
use ratatui::style::{Modifier, Style};

pub use crate::ops::cargo_tree::{Charset as TreeCharset, Prefix as TreePrefix};

/// Visual configuration for [`TreeWidget`](super::TreeWidget).
#[derive(Debug)]
pub struct TreeWidgetStyle {
    pub highlight_style: Style,
    pub filtered_style: Style,
    pub style: Style,
    pub context_style: Style,
    pub name_style: Style,
    pub version_style: Style,
    pub suffix_style: Style,
    pub node_symbol: char,
    pub node_closed_symbol: char,
    pub node_open_symbol: char,
    pub branch_symbol: &'static str,
    pub last_branch_symbol: &'static str,
    pub continuation_symbol: &'static str,
    pub empty_symbol: &'static str,
}

impl Default for TreeWidgetStyle {
    fn default() -> Self {
        Self {
            highlight_style: Style::from(WARN).add_modifier(Modifier::BOLD),
            filtered_style: {
                let style: Style = DEP_FEATURE.into();
                style.remove_modifier(Modifier::DIM)
            },
            style: NOP.into(),
            context_style: Modifier::DIM.into(),
            name_style: NOP.into(),
            version_style: PLACEHOLDER.into(),
            suffix_style: DEP_BUILD.into(),
            node_symbol: '•',
            node_closed_symbol: '▸',
            node_open_symbol: '▾',
            branch_symbol: "├──",
            last_branch_symbol: "└──",
            continuation_symbol: "│  ",
            empty_symbol: "   ",
        }
    }
}

impl TreeWidgetStyle {
    pub fn set_charset(&mut self, charset: TreeCharset) {
        match charset {
            TreeCharset::Utf8 => {
                self.node_symbol = '•';
                self.node_closed_symbol = '▸';
                self.node_open_symbol = '▾';
                self.branch_symbol = "├──";
                self.last_branch_symbol = "└──";
                self.continuation_symbol = "│  ";
                self.empty_symbol = "   ";
            }
            TreeCharset::Ascii => {
                self.node_symbol = '*';
                self.node_closed_symbol = '>';
                self.node_open_symbol = 'v';
                self.branch_symbol = "|-- ";
                self.last_branch_symbol = "`-- ";
                self.continuation_symbol = "|   ";
                self.empty_symbol = "    ";
            }
        }
    }
}
