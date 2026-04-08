use super::sidebar_model::{
    SidebarChildren, SidebarIcon, SidebarNode, SidebarTree, connection_node_id, provider_node_id,
};
use crate::{config::ResolvedConnectionProfile, i18n::I18n};
use gpui::Context;
use kandb_assets::{IconName, ProviderIconName};
use kandb_i18n::FluentArgs;
use kandb_provider_core::{Connection, IconToken, ProviderRegistry, TreeChildren, TreeNode};
use kandb_provider_sqlite::SqlitePlugin;
use std::sync::Arc;

pub(crate) struct SidebarState {
    connections: Vec<ConnectionEntry>,
}

struct ConnectionEntry {
    profile: ResolvedConnectionProfile,
    generation: u64,
    status: LoadState,
}

enum LoadState {
    Unloaded,
    Loading,
    Loaded(LoadedConnection),
    Unsupported(String),
    Error(String),
}

struct LoadedConnection {
    tree: kandb_provider_core::SidebarTree,
}

impl SidebarState {
    pub(crate) fn from_config(config: &[ResolvedConnectionProfile]) -> Self {
        Self {
            connections: config
                .iter()
                .cloned()
                .map(|profile| ConnectionEntry {
                    profile,
                    generation: 0,
                    status: LoadState::Unloaded,
                })
                .collect(),
        }
    }

    pub(crate) fn preload_all_connections(&mut self, locale: &str, cx: &mut Context<Self>) {
        for connection_index in 0..self.connections.len() {
            self.ensure_connection_loaded(connection_index, locale, cx);
        }
    }

    pub(crate) fn refresh_connection(
        &mut self,
        target_connection_node_id: &str,
        locale: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(connection_index) = self.connections.iter().position(|connection| {
            connection_node_id(&connection.profile.id) == target_connection_node_id
        }) else {
            return;
        };

        self.connections[connection_index].generation += 1;
        self.connections[connection_index].status = LoadState::Unloaded;
        self.ensure_connection_loaded(connection_index, locale, cx);
    }

    pub(crate) fn refresh_all_connections(&mut self, locale: &str, cx: &mut Context<Self>) {
        for connection_index in 0..self.connections.len() {
            self.connections[connection_index].generation += 1;
            self.connections[connection_index].status = LoadState::Unloaded;
            self.ensure_connection_loaded(connection_index, locale, cx);
        }
    }

    pub(crate) fn is_connection_refreshing(&self, target_connection_node_id: &str) -> bool {
        self.connections.iter().any(|connection| {
            connection_node_id(&connection.profile.id) == target_connection_node_id
                && matches!(connection.status, LoadState::Loading)
        })
    }

    pub(crate) fn is_any_refreshing(&self) -> bool {
        self.connections
            .iter()
            .any(|connection| matches!(connection.status, LoadState::Loading))
    }

    pub(crate) fn build_tree(&self, i18n: &I18n) -> SidebarTree {
        SidebarTree::new(
            self.connections
                .iter()
                .map(|connection| build_connection_node(connection, i18n))
                .collect(),
        )
    }

    fn ensure_connection_loaded(
        &mut self,
        connection_index: usize,
        locale: &str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(
            self.connections[connection_index].status,
            LoadState::Unloaded
        ) {
            return;
        }

        self.connections[connection_index].status = LoadState::Loading;
        let weak = cx.entity().downgrade();
        let profile = self.connections[connection_index].profile.clone();
        let generation = self.connections[connection_index].generation;
        let locale = locale.to_string();

        cx.spawn(async move |_, cx| {
            let result = connect_and_load_sidebar(profile.clone(), &locale).await;
            let _ = weak.update(cx, |state, cx| {
                let Some(connection_index) = state.connections.iter().position(|connection| {
                    connection.profile.id == profile.id && connection.generation == generation
                }) else {
                    return;
                };

                state.connections[connection_index].status = match result {
                    Ok((_connection, tree)) => LoadState::Loaded(LoadedConnection { tree }),
                    Err(LoadFailure::Unsupported(provider)) => LoadState::Unsupported(provider),
                    Err(LoadFailure::Error(message)) => LoadState::Error(message),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn build_connection_node(connection: &ConnectionEntry, i18n: &I18n) -> SidebarNode {
    let id = connection_node_id(&connection.profile.id);
    SidebarNode {
        id: id.clone(),
        label: connection.profile.name.clone(),
        icon: provider_icon(&connection.profile.provider_kind),
        parent_id: None,
        selectable: true,
        children: SidebarChildren::Branch(match &connection.status {
            LoadState::Unloaded => Vec::new(),
            LoadState::Loading => vec![message_node(
                &id,
                "connection:loading",
                i18n.t("app-home-sidebar-loading"),
            )],
            LoadState::Loaded(loaded) => loaded
                .tree
                .roots
                .iter()
                .map(|node| map_provider_tree_node(&connection.profile.id, &id, node))
                .collect(),
            LoadState::Unsupported(provider) => vec![message_node(
                &id,
                "connection:unsupported",
                i18n.t_with_args("app-home-sidebar-provider-unsupported", &{
                    let mut args = FluentArgs::new();
                    args.set("provider", provider.as_str());
                    args
                }),
            )],
            LoadState::Error(message) => {
                vec![message_node(&id, "connection:error", message.clone())]
            }
        }),
    }
}

fn map_provider_tree_node(connection_id: &str, parent_id: &str, node: &TreeNode) -> SidebarNode {
    let node_id = provider_node_id(connection_id, &node.id);
    SidebarNode {
        id: node_id.clone(),
        label: node.label.clone(),
        icon: map_icon(node.icon),
        parent_id: Some(parent_id.to_string()),
        selectable: true,
        children: match &node.children {
            TreeChildren::Leaf => SidebarChildren::Leaf,
            TreeChildren::Branch(children) => SidebarChildren::Branch(
                children
                    .iter()
                    .map(|child| map_provider_tree_node(connection_id, &node_id, child))
                    .collect(),
            ),
        },
    }
}

fn map_icon(icon: IconToken) -> SidebarIcon {
    match icon {
        IconToken::Database => SidebarIcon::Lucide(IconName::Database),
        IconToken::Folder => SidebarIcon::Folder,
        IconToken::HardDrive => SidebarIcon::Lucide(IconName::HardDrive),
        IconToken::Table => SidebarIcon::Lucide(IconName::Table),
        IconToken::View => SidebarIcon::Lucide(IconName::Rows3),
        IconToken::Column => SidebarIcon::Lucide(IconName::Hash),
        IconToken::Key => SidebarIcon::Lucide(IconName::KeyRound),
        IconToken::Index => SidebarIcon::Lucide(IconName::ListTree),
    }
}

fn provider_icon(provider_kind: &str) -> SidebarIcon {
    match provider_kind {
        "sqlite" => SidebarIcon::Provider(ProviderIconName::Sqlite),
        _ => SidebarIcon::Lucide(IconName::Database),
    }
}

fn message_node(parent_id: &str, suffix: &str, label: String) -> SidebarNode {
    SidebarNode {
        id: format!("{parent_id}:{suffix}"),
        label,
        icon: SidebarIcon::Lucide(IconName::SquareTerminal),
        parent_id: Some(parent_id.to_string()),
        selectable: false,
        children: SidebarChildren::Leaf,
    }
}

enum LoadFailure {
    Unsupported(String),
    Error(String),
}

async fn connect_and_load_sidebar(
    profile: ResolvedConnectionProfile,
    locale: &str,
) -> Result<(Arc<dyn Connection>, kandb_provider_core::SidebarTree), LoadFailure> {
    let registry = provider_registry();
    let Some(plugin) = registry.get(&profile.provider_kind) else {
        return Err(LoadFailure::Unsupported(profile.provider_kind));
    };

    let connection = plugin
        .connect_erased(profile.config_json.clone())
        .await
        .map_err(|error| LoadFailure::Error(error.to_string()))?;
    let connection: Arc<dyn Connection> = Arc::from(connection);
    let tree = connection
        .load_sidebar(locale)
        .await
        .map_err(|error| LoadFailure::Error(error.to_string()))?;

    Ok((connection, tree))
}

fn provider_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(Arc::new(SqlitePlugin))
        .expect("sqlite provider should register exactly once");
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConnectionProfile;

    #[test]
    fn unsupported_provider_is_rendered_as_message_child() {
        let state = SidebarState {
            connections: vec![ConnectionEntry {
                profile: ResolvedConnectionProfile {
                    id: "redis-local".into(),
                    name: "Redis Local".into(),
                    provider_kind: "redis".into(),
                    config_json: serde_json::json!({}),
                },
                generation: 0,
                status: LoadState::Unsupported("redis".into()),
            }],
        };

        let tree = state.build_tree(&I18n::english_for_test());
        let visible = tree.visible_nodes(&std::collections::BTreeSet::from([connection_node_id(
            "redis-local",
        )]));

        assert!(visible.iter().any(|node| node.label.contains("redis")));
    }
}
