use gpui::SharedString;
use kandb_assets::{IconName, ProviderIconName};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SidebarNodeKind {
    Connection,
    Namespace,
    ResourceBucket,
    Resource,
    ResourceChildBucket,
    Field,
    Key,
    Index,
    Loading,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarBucketKind {
    Tables,
    Views,
    Columns,
    Keys,
    Indexes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarIcon {
    Lucide(IconName),
    Provider(ProviderIconName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidebarNode {
    pub(crate) id: String,
    pub(crate) label: SharedString,
    pub(crate) kind: SidebarNodeKind,
    pub(crate) icon: SidebarIcon,
    pub(crate) parent_id: Option<String>,
    pub(crate) children: Vec<SidebarNode>,
    pub(crate) trailing_label: Option<SharedString>,
    pub(crate) badge_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisibleSidebarNode {
    pub(crate) id: String,
    pub(crate) label: SharedString,
    pub(crate) kind: SidebarNodeKind,
    pub(crate) icon: SidebarIcon,
    pub(crate) parent_id: Option<String>,
    pub(crate) depth: usize,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) trailing_label: Option<SharedString>,
    pub(crate) badge_count: Option<usize>,
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

    pub(crate) fn default_selected_node_id(
        &self,
        preferred_connection_id: Option<&str>,
    ) -> Option<&str> {
        self.preferred_root(preferred_connection_id)
            .map(|node| node.id.as_str())
    }

    pub(crate) fn default_expanded_node_ids(
        &self,
        preferred_connection_id: Option<&str>,
    ) -> BTreeSet<String> {
        let mut expanded = BTreeSet::new();
        if let Some(root) = self.preferred_root(preferred_connection_id) {
            expanded.insert(root.id.clone());
            if let Some(first_namespace) = root
                .children
                .iter()
                .find(|child| matches!(child.kind, SidebarNodeKind::Namespace))
            {
                expanded.insert(first_namespace.id.clone());
                if let Some(first_bucket) = first_namespace
                    .children
                    .iter()
                    .find(|child| matches!(child.kind, SidebarNodeKind::ResourceBucket))
                {
                    expanded.insert(first_bucket.id.clone());
                }
            }
        }
        expanded
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

    pub(crate) fn is_connection_node(&self, node_id: &str) -> bool {
        self.find_node(node_id)
            .is_some_and(|node| matches!(node.kind, SidebarNodeKind::Connection))
    }

    pub(crate) fn connection_node_id_for(&self, node_id: &str) -> Option<&str> {
        self.roots
            .iter()
            .find_map(|root| find_connection_node_id(root, node_id))
    }

    fn find_node(&self, node_id: &str) -> Option<&SidebarNode> {
        self.roots.iter().find_map(|root| find_node(root, node_id))
    }

    fn preferred_root(&self, preferred_connection_id: Option<&str>) -> Option<&SidebarNode> {
        preferred_connection_id
            .and_then(|connection_id| {
                let node_id = format!("connection:{connection_id}");
                self.roots.iter().find(|root| root.id == node_id)
            })
            .or_else(|| self.roots.first())
    }
}

fn collect_node_ids(node: &SidebarNode, ids: &mut BTreeSet<String>) {
    ids.insert(node.id.clone());
    for child in &node.children {
        collect_node_ids(child, ids);
    }
}

fn append_visible_nodes(
    node: &SidebarNode,
    depth: usize,
    expanded_node_ids: &BTreeSet<String>,
    visible: &mut Vec<VisibleSidebarNode>,
) {
    let expanded = expanded_node_ids.contains(&node.id);
    visible.push(VisibleSidebarNode {
        id: node.id.clone(),
        label: node.label.clone(),
        kind: node.kind.clone(),
        icon: node.icon,
        parent_id: node.parent_id.clone(),
        depth,
        expandable: !node.children.is_empty(),
        expanded,
        trailing_label: node.trailing_label.clone(),
        badge_count: node.badge_count,
    });

    if !expanded {
        return;
    }

    for child in &node.children {
        append_visible_nodes(child, depth + 1, expanded_node_ids, visible);
    }
}

fn find_node<'a>(node: &'a SidebarNode, node_id: &str) -> Option<&'a SidebarNode> {
    if node.id == node_id {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_node(child, node_id))
}

fn find_connection_node_id<'a>(node: &'a SidebarNode, target_id: &str) -> Option<&'a str> {
    let is_connection = matches!(node.kind, SidebarNodeKind::Connection);
    if node.id == target_id {
        return is_connection.then_some(node.id.as_str());
    }

    if node
        .children
        .iter()
        .any(|child| find_node(child, target_id).is_some())
    {
        return is_connection.then_some(node.id.as_str());
    }

    None
}
