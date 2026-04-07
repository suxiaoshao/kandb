use crate::config::{LoadedAppConfig, ResolvedConnectionProfile, ResolvedProviderConfig};
use gpui::SharedString;
use kandb_assets::{IconName, ProviderIconName};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SidebarNodeKind {
    Connection,
    Namespace,
    ResourceGroup,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidebarTree {
    roots: Vec<SidebarNode>,
    default_connection_id: Option<String>,
}

impl SidebarTree {
    pub(crate) fn from_config(config: &LoadedAppConfig) -> Self {
        Self {
            default_connection_id: config.file.default_connection_id.clone(),
            roots: config
                .resolved_connections
                .iter()
                .map(build_connection_node)
                .collect(),
        }
    }

    pub(crate) fn valid_node_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for root in &self.roots {
            collect_node_ids(root, &mut ids);
        }
        ids
    }

    pub(crate) fn default_selected_node_id(&self) -> Option<&str> {
        if let Some(default_connection_id) = self.default_connection_id.as_deref() {
            let preferred_id = format!("connection:{default_connection_id}");
            if let Some(node) = self.roots.iter().find(|node| node.id == preferred_id) {
                return Some(node.id.as_str());
            }
        }

        self.roots.first().map(|node| node.id.as_str())
    }

    pub(crate) fn default_expanded_node_ids(&self) -> BTreeSet<String> {
        let mut expanded = BTreeSet::new();
        if let Some(root) = self.roots.iter().find(|root| !root.children.is_empty()) {
            expanded.insert(root.id.clone());
            if let Some(namespace) = root.children.first() {
                expanded.insert(namespace.id.clone());
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
}

fn build_connection_node(connection: &ResolvedConnectionProfile) -> SidebarNode {
    let id = format!("connection:{}", connection.id);
    let parent_id = Some(id.clone());
    let children = match &connection.provider {
        ResolvedProviderConfig::Sqlite(_) => vec![SidebarNode {
            id: format!("namespace:{}:main", connection.id),
            label: "main".into(),
            kind: SidebarNodeKind::Namespace,
            icon: SidebarIcon::Lucide(IconName::HardDrive),
            parent_id,
            children: vec![
                group_node(&connection.id, "tables", "Tables", IconName::Table),
                group_node(&connection.id, "views", "Views", IconName::Rows3),
                group_node(&connection.id, "system", "System", IconName::SquareTerminal),
            ],
        }],
        ResolvedProviderConfig::Unknown { .. } => Vec::new(),
    };

    SidebarNode {
        id,
        label: connection.name.clone().into(),
        kind: SidebarNodeKind::Connection,
        icon: match &connection.provider {
            ResolvedProviderConfig::Sqlite(_) => SidebarIcon::Provider(ProviderIconName::Sqlite),
            ResolvedProviderConfig::Unknown { .. } => SidebarIcon::Lucide(IconName::Database),
        },
        parent_id: None,
        children,
    }
}

fn group_node(connection_id: &str, slug: &str, label: &'static str, icon: IconName) -> SidebarNode {
    SidebarNode {
        id: format!("group:{connection_id}:main:{slug}"),
        label: label.into(),
        kind: SidebarNodeKind::ResourceGroup,
        icon: SidebarIcon::Lucide(icon),
        parent_id: Some(format!("namespace:{connection_id}:main")),
        children: Vec::new(),
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
    });

    if !expanded {
        return;
    }

    for child in &node.children {
        append_visible_nodes(child, depth + 1, expanded_node_ids, visible);
    }
}

#[cfg(test)]
mod tests {
    use super::{SidebarNodeKind, SidebarTree};
    use crate::{
        app_paths::AppPaths,
        config::{
            AppConfigFile, LoadedAppConfig, ResolvedConnectionProfile, ResolvedProviderConfig,
            StoredConnectionProfile,
        },
    };
    use kandb_provider_sqlite::{SqliteConfig, SqliteLocation};
    use std::{collections::BTreeSet, path::PathBuf};

    fn sample_config() -> LoadedAppConfig {
        LoadedAppConfig {
            paths: AppPaths::from_roots(PathBuf::from("/tmp/config"), PathBuf::from("/tmp/data")),
            file: AppConfigFile {
                version: 1,
                default_connection_id: Some("local-main".to_owned()),
                connections: vec![StoredConnectionProfile {
                    id: "local-main".to_owned(),
                    name: "Local Main".to_owned(),
                    provider: "sqlite".to_owned(),
                    config: toml::Table::new(),
                }],
            },
            resolved_connections: vec![ResolvedConnectionProfile {
                id: "local-main".to_owned(),
                name: "Local Main".to_owned(),
                provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                    location: SqliteLocation::Memory,
                    read_only: false,
                    create_if_missing: true,
                }),
            }],
        }
    }

    #[test]
    fn sqlite_connections_project_to_synthetic_tree() {
        let tree = SidebarTree::from_config(&sample_config());
        let visible = tree.visible_nodes(&tree.default_expanded_node_ids());

        assert_eq!(visible[0].id, "connection:local-main");
        assert_eq!(visible[1].id, "namespace:local-main:main");
        assert_eq!(visible[2].id, "group:local-main:main:tables");
        assert_eq!(visible[3].kind, SidebarNodeKind::ResourceGroup);
    }

    #[test]
    fn valid_node_ids_include_namespace_and_groups() {
        let tree = SidebarTree::from_config(&sample_config());
        let ids = tree.valid_node_ids();

        assert!(ids.contains("connection:local-main"));
        assert!(ids.contains("namespace:local-main:main"));
        assert!(ids.contains("group:local-main:main:views"));
    }

    #[test]
    fn visible_nodes_respect_expansion_state() {
        let tree = SidebarTree::from_config(&sample_config());
        let visible = tree.visible_nodes(&BTreeSet::from(["connection:local-main".to_owned()]));

        assert_eq!(visible.len(), 2);
        assert_eq!(visible[1].id, "namespace:local-main:main");
    }

    #[test]
    fn default_selection_prefers_configured_default_connection() {
        let mut config = sample_config();
        config.file.default_connection_id = Some("secondary".to_owned());
        config.resolved_connections = vec![
            ResolvedConnectionProfile {
                id: "local-main".to_owned(),
                name: "Local Main".to_owned(),
                provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                    location: SqliteLocation::Memory,
                    read_only: false,
                    create_if_missing: true,
                }),
            },
            ResolvedConnectionProfile {
                id: "secondary".to_owned(),
                name: "Secondary".to_owned(),
                provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                    location: SqliteLocation::Memory,
                    read_only: false,
                    create_if_missing: true,
                }),
            },
        ];

        let tree = SidebarTree::from_config(&config);

        assert_eq!(
            tree.default_selected_node_id(),
            Some("connection:secondary")
        );
    }
}
