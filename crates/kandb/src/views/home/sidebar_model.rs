use kandb_assets::{IconName, ProviderIconName};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarIcon {
    Lucide(IconName),
    Provider(ProviderIconName),
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidebarNode {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) icon: SidebarIcon,
    pub(crate) parent_id: Option<String>,
    pub(crate) children: SidebarChildren,
    pub(crate) selectable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SidebarChildren {
    Leaf,
    Branch(Vec<SidebarNode>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisibleSidebarNode {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) icon: SidebarIcon,
    pub(crate) parent_id: Option<String>,
    pub(crate) depth: usize,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) selectable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidebarTree {
    roots: Vec<SidebarNode>,
}

impl SidebarTree {
    pub(crate) fn new(roots: Vec<SidebarNode>) -> Self {
        Self { roots }
    }

    pub(crate) fn valid_node_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for root in &self.roots {
            collect_node_ids(root, &mut ids);
        }
        ids
    }

    pub(crate) fn visible_nodes(
        &self,
        expanded_node_ids: &BTreeSet<String>,
    ) -> Vec<VisibleSidebarNode> {
        let mut visible = Vec::new();
        for root in &self.roots {
            append_visible_nodes(root, 0, expanded_node_ids, &mut visible);
        }
        visible
    }

    pub(crate) fn find_visible_index(
        &self,
        expanded_node_ids: &BTreeSet<String>,
        selected_node_id: Option<&str>,
    ) -> Option<usize> {
        let visible = self.visible_nodes(expanded_node_ids);
        selected_node_id.and_then(|selected| visible.iter().position(|node| node.id == selected))
    }

    pub(crate) fn find_visible_node(
        &self,
        expanded_node_ids: &BTreeSet<String>,
        node_id: &str,
    ) -> Option<VisibleSidebarNode> {
        self.visible_nodes(expanded_node_ids)
            .into_iter()
            .find(|node| node.id == node_id)
    }

    pub(crate) fn connection_node_id_for(&self, node_id: &str) -> Option<&str> {
        self.roots
            .iter()
            .find_map(|root| find_connection_node_id(root, node_id))
    }
}

pub(crate) fn connection_node_id(connection_id: &str) -> String {
    format!("connection:{connection_id}")
}

pub(crate) fn provider_node_id(connection_id: &str, provider_node_id: &str) -> String {
    format!("connection:{connection_id}/node:{provider_node_id}")
}

fn collect_node_ids(node: &SidebarNode, ids: &mut BTreeSet<String>) {
    ids.insert(node.id.clone());
    if let SidebarChildren::Branch(children) = &node.children {
        for child in children {
            collect_node_ids(child, ids);
        }
    }
}

fn append_visible_nodes(
    node: &SidebarNode,
    depth: usize,
    expanded_node_ids: &BTreeSet<String>,
    visible: &mut Vec<VisibleSidebarNode>,
) {
    let expanded = expanded_node_ids.contains(&node.id);
    let expandable = matches!(node.children, SidebarChildren::Branch(_));
    visible.push(VisibleSidebarNode {
        id: node.id.clone(),
        label: node.label.clone(),
        icon: node.icon,
        parent_id: node.parent_id.clone(),
        depth,
        expandable,
        expanded,
        selectable: node.selectable,
    });

    if !expanded {
        return;
    }

    if let SidebarChildren::Branch(children) = &node.children {
        for child in children {
            append_visible_nodes(child, depth + 1, expanded_node_ids, visible);
        }
    }
}

fn find_connection_node_id<'a>(node: &'a SidebarNode, target_id: &str) -> Option<&'a str> {
    if node.id == target_id {
        return Some(node.id.as_str());
    }

    match &node.children {
        SidebarChildren::Leaf => None,
        SidebarChildren::Branch(children) => {
            if children.iter().any(|child| contains_node(child, target_id)) {
                Some(node.id.as_str())
            } else {
                None
            }
        }
    }
}

fn contains_node(node: &SidebarNode, target_id: &str) -> bool {
    if node.id == target_id {
        return true;
    }

    match &node.children {
        SidebarChildren::Leaf => false,
        SidebarChildren::Branch(children) => {
            children.iter().any(|child| contains_node(child, target_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> SidebarTree {
        SidebarTree::new(vec![SidebarNode {
            id: connection_node_id("local"),
            label: "Local".into(),
            icon: SidebarIcon::Provider(ProviderIconName::Sqlite),
            parent_id: None,
            selectable: true,
            children: SidebarChildren::Branch(vec![SidebarNode {
                id: provider_node_id("local", "namespace:main"),
                label: "main".into(),
                icon: SidebarIcon::Lucide(IconName::HardDrive),
                parent_id: Some(connection_node_id("local")),
                selectable: true,
                children: SidebarChildren::Branch(vec![SidebarNode {
                    id: provider_node_id("local", "group:tables"),
                    label: "Tables".into(),
                    icon: SidebarIcon::Folder,
                    parent_id: Some(provider_node_id("local", "namespace:main")),
                    selectable: true,
                    children: SidebarChildren::Branch(Vec::new()),
                }]),
            }]),
        }])
    }

    #[test]
    fn refresh_target_uses_selected_connection() {
        let tree = sample_tree();
        let connection_id = connection_node_id("local");

        assert_eq!(
            tree.connection_node_id_for(&connection_id),
            Some(connection_id.as_str())
        );
    }

    #[test]
    fn refresh_target_resolves_nested_node_to_connection() {
        let tree = sample_tree();
        let nested_id = provider_node_id("local", "group:tables");
        let connection_id = connection_node_id("local");

        assert_eq!(
            tree.connection_node_id_for(&nested_id),
            Some(connection_id.as_str())
        );
    }

    #[test]
    fn branch_nodes_are_expandable_even_when_empty() {
        let tree = sample_tree();
        let visible = tree.visible_nodes(&BTreeSet::from([connection_node_id("local")]));

        assert!(visible.iter().any(|node| node.expandable));
    }
}
