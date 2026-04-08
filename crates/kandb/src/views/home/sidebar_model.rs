use gpui::SharedString;
use kandb_assets::{IconName, ProviderIconName};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SidebarNodeId {
    Connection {
        connection_id: String,
    },
    Namespace {
        connection_id: String,
        namespace_id: String,
    },
    ResourceBucket {
        connection_id: String,
        namespace_id: String,
        bucket: SidebarBucketKind,
    },
    Resource {
        connection_id: String,
        namespace_id: String,
        resource_name: String,
    },
    ResourceChildBucket {
        connection_id: String,
        namespace_id: String,
        resource_name: String,
        bucket: SidebarBucketKind,
    },
    Field {
        connection_id: String,
        namespace_id: String,
        resource_name: String,
        field_name: String,
    },
    Key {
        connection_id: String,
        namespace_id: String,
        resource_name: String,
        key_signature: String,
    },
    Index {
        connection_id: String,
        namespace_id: String,
        resource_name: String,
        index_signature: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidebarEphemeralRow {
    pub(crate) id: String,
    pub(crate) label: SharedString,
    pub(crate) kind: SidebarNodeKind,
    pub(crate) icon: SidebarIcon,
    pub(crate) parent_id: String,
    pub(crate) trailing_label: Option<SharedString>,
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
    pub(crate) ephemeral_child: Option<SidebarEphemeralRow>,
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
            .map(connection_node_id)
            .and_then(|node_id| self.roots.iter().find(|root| root.id == node_id))
            .or_else(|| self.roots.first())
    }
}

pub(crate) fn encode_node_id(node_id: &SidebarNodeId) -> String {
    let encoded = serde_json::to_vec(node_id).expect("sidebar node id must serialize");
    format!("sid:{}", hex_encode(&encoded))
}

pub(crate) fn decode_node_id(node_id: &str) -> Option<SidebarNodeId> {
    let encoded = node_id.strip_prefix("sid:")?;
    let bytes = hex_decode(encoded)?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn migrate_legacy_node_id(node_id: &str) -> Option<String> {
    let migrated = if let Some(connection_id) = node_id.strip_prefix("connection:") {
        SidebarNodeId::Connection {
            connection_id: connection_id.to_string(),
        }
    } else if let Some(rest) = node_id.strip_prefix("namespace:") {
        let mut parts = rest.split(':');
        let migrated = SidebarNodeId::Namespace {
            connection_id: parts.next()?.to_string(),
            namespace_id: parts.next()?.to_string(),
        };
        if parts.next().is_some() {
            return None;
        }
        migrated
    } else if let Some(rest) = node_id.strip_prefix("bucket:") {
        let mut parts = rest.split(':');
        let migrated = SidebarNodeId::ResourceBucket {
            connection_id: parts.next()?.to_string(),
            namespace_id: parts.next()?.to_string(),
            bucket: parse_bucket_slug(parts.next()?)?,
        };
        if parts.next().is_some() {
            return None;
        }
        migrated
    } else if let Some(rest) = node_id.strip_prefix("child-bucket:") {
        let mut parts = rest.split(':');
        let migrated = SidebarNodeId::ResourceChildBucket {
            connection_id: parts.next()?.to_string(),
            namespace_id: parts.next()?.to_string(),
            resource_name: parts.next()?.to_string(),
            bucket: parse_bucket_slug(parts.next()?)?,
        };
        if parts.next().is_some() {
            return None;
        }
        migrated
    } else if let Some(rest) = node_id.strip_prefix("resource:") {
        let mut parts = rest.split(':');
        let migrated = SidebarNodeId::Resource {
            connection_id: parts.next()?.to_string(),
            namespace_id: parts.next()?.to_string(),
            resource_name: parts.next()?.to_string(),
        };
        if parts.next().is_some() {
            return None;
        }
        migrated
    } else if let Some(rest) = node_id.strip_prefix("field:") {
        let mut parts = rest.split(':');
        let migrated = SidebarNodeId::Field {
            connection_id: parts.next()?.to_string(),
            namespace_id: parts.next()?.to_string(),
            resource_name: parts.next()?.to_string(),
            field_name: parts.next()?.to_string(),
        };
        if parts.next().is_some() {
            return None;
        }
        migrated
    } else {
        return None;
    };

    Some(encode_node_id(&migrated))
}

pub(crate) fn persisted_connection_node_id(node_id: &str) -> Option<String> {
    match decode_node_id(node_id) {
        Some(SidebarNodeId::Connection { connection_id })
        | Some(SidebarNodeId::Namespace { connection_id, .. })
        | Some(SidebarNodeId::ResourceBucket { connection_id, .. })
        | Some(SidebarNodeId::Resource { connection_id, .. })
        | Some(SidebarNodeId::ResourceChildBucket { connection_id, .. })
        | Some(SidebarNodeId::Field { connection_id, .. })
        | Some(SidebarNodeId::Key { connection_id, .. })
        | Some(SidebarNodeId::Index { connection_id, .. }) => Some(connection_node_id(&connection_id)),
        None => {
            if let Some(node_id) = migrate_legacy_node_id(node_id) {
                persisted_connection_node_id(&node_id)
            } else {
                None
            }
        }
    }
}

pub(crate) fn connection_node_id(connection_id: &str) -> String {
    encode_node_id(&SidebarNodeId::Connection {
        connection_id: connection_id.to_string(),
    })
}

pub(crate) fn namespace_node_id(connection_id: &str, namespace_id: &str) -> String {
    encode_node_id(&SidebarNodeId::Namespace {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
    })
}

pub(crate) fn bucket_node_id(
    connection_id: &str,
    namespace_id: &str,
    bucket: SidebarBucketKind,
) -> String {
    encode_node_id(&SidebarNodeId::ResourceBucket {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
        bucket,
    })
}

pub(crate) fn child_bucket_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    bucket: SidebarBucketKind,
) -> String {
    encode_node_id(&SidebarNodeId::ResourceChildBucket {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
        resource_name: resource_name.to_string(),
        bucket,
    })
}

pub(crate) fn resource_node_id(connection_id: &str, namespace_id: &str, resource_name: &str) -> String {
    encode_node_id(&SidebarNodeId::Resource {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
        resource_name: resource_name.to_string(),
    })
}

pub(crate) fn field_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    field_name: &str,
) -> String {
    encode_node_id(&SidebarNodeId::Field {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
        resource_name: resource_name.to_string(),
        field_name: field_name.to_string(),
    })
}

pub(crate) fn key_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    key_signature: &str,
) -> String {
    encode_node_id(&SidebarNodeId::Key {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
        resource_name: resource_name.to_string(),
        key_signature: key_signature.to_string(),
    })
}

pub(crate) fn index_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    index_signature: &str,
) -> String {
    encode_node_id(&SidebarNodeId::Index {
        connection_id: connection_id.to_string(),
        namespace_id: namespace_id.to_string(),
        resource_name: resource_name.to_string(),
        index_signature: index_signature.to_string(),
    })
}

pub(crate) fn key_signature(kind: &str, name: Option<&str>, columns: &[String]) -> String {
    format!("{kind}|{}|{}", name.unwrap_or(""), columns.join("\u{1f}"))
}

pub(crate) fn index_signature(name: &str, columns: &[String]) -> String {
    format!("{name}|{}", columns.join("\u{1f}"))
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
        expandable: !node.children.is_empty() || node.ephemeral_child.is_some(),
        expanded,
        trailing_label: node.trailing_label.clone(),
        badge_count: node.badge_count,
        selectable: true,
    });

    if !expanded {
        return;
    }

    for child in &node.children {
        append_visible_nodes(child, depth + 1, expanded_node_ids, visible);
    }

    if let Some(ephemeral) = &node.ephemeral_child {
        visible.push(VisibleSidebarNode {
            id: ephemeral.id.clone(),
            label: ephemeral.label.clone(),
            kind: ephemeral.kind.clone(),
            icon: ephemeral.icon,
            parent_id: Some(ephemeral.parent_id.clone()),
            depth: depth + 1,
            expandable: false,
            expanded: false,
            trailing_label: ephemeral.trailing_label.clone(),
            badge_count: None,
            selectable: false,
        });
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

fn parse_bucket_slug(slug: &str) -> Option<SidebarBucketKind> {
    match slug {
        "tables" => Some(SidebarBucketKind::Tables),
        "views" => Some(SidebarBucketKind::Views),
        "columns" => Some(SidebarBucketKind::Columns),
        "keys" => Some(SidebarBucketKind::Keys),
        "indexes" => Some(SidebarBucketKind::Indexes),
        _ => None,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(hex_char(byte >> 4));
        out.push(hex_char(byte & 0x0f));
    }
    out
}

fn hex_decode(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }

    input
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let high = hex_value(chunk[0])?;
            let low = hex_value(chunk[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("hex nibble out of range"),
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_ids_roundtrip_through_stable_encoding() {
        let node_id = SidebarNodeId::Field {
            connection_id: "local".into(),
            namespace_id: "main".into(),
            resource_name: "users:loading".into(),
            field_name: "name:error".into(),
        };

        let encoded = encode_node_id(&node_id);
        assert_eq!(decode_node_id(&encoded), Some(node_id));
    }

    #[test]
    fn legacy_ids_migrate_when_components_are_unambiguous() {
        let migrated = migrate_legacy_node_id("resource:local:main:users").unwrap();
        assert_eq!(
            decode_node_id(&migrated),
            Some(SidebarNodeId::Resource {
                connection_id: "local".into(),
                namespace_id: "main".into(),
                resource_name: "users".into(),
            })
        );
    }

    #[test]
    fn legacy_ids_with_ambiguous_components_do_not_migrate() {
        assert!(migrate_legacy_node_id("resource:local:main:users:loading").is_none());
    }
}
