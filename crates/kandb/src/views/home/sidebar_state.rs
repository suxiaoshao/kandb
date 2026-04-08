use super::sidebar_model::{
    SidebarBucketKind, SidebarIcon, SidebarNode, SidebarNodeKind, SidebarTree,
};
use crate::{
    config::{LoadedAppConfig, ResolvedConnectionProfile, ResolvedProviderConfig},
    i18n::I18n,
};
use gpui::Context;
use kandb_assets::{IconName, ProviderIconName};
use kandb_provider_core::{
    Connection, FieldMeta, NamespaceInfo, ProviderFactory, ResourceIndexInfo, ResourceInfo,
    ResourceKeyInfo, ResourceKind, ResourceRef,
};
use kandb_provider_sqlite::SqliteProvider;
use std::collections::BTreeSet;

pub(crate) struct SidebarState {
    connections: Vec<ConnectionEntry>,
}

struct ConnectionEntry {
    profile: ResolvedConnectionProfile,
    generation: u64,
    status: LoadState<Vec<NamespaceEntry>>,
}

struct NamespaceEntry {
    info: NamespaceInfo,
    resources: LoadState<Vec<ResourceEntry>>,
}

struct ResourceEntry {
    info: ResourceInfo,
    structure: LoadState<ResourceStructure>,
}

struct ResourceStructure {
    fields: Vec<FieldMeta>,
    keys: Vec<ResourceKeyInfo>,
    indexes: Vec<ResourceIndexInfo>,
}

enum LoadState<T> {
    Unloaded,
    Loading,
    Loaded(T),
    Error(String),
    Unsupported(String),
}

impl SidebarState {
    pub(crate) fn from_config(config: &LoadedAppConfig) -> Self {
        Self {
            connections: config
                .resolved_connections
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

    pub(crate) fn preload_all_connections(&mut self, cx: &mut Context<Self>) {
        for connection_index in 0..self.connections.len() {
            self.ensure_connection_preloaded(connection_index, cx);
        }
    }

    pub(crate) fn refresh_connection(
        &mut self,
        target_connection_node_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(connection_index) = self
            .connections
            .iter()
            .position(|connection| {
                connection_node_id(&connection.profile.id) == target_connection_node_id
            })
        else {
            return;
        };

        self.reset_connection(connection_index);
        self.ensure_connection_preloaded(connection_index, cx);
    }

    pub(crate) fn refresh_all_connections(&mut self, cx: &mut Context<Self>) {
        for connection_index in 0..self.connections.len() {
            self.reset_connection(connection_index);
            self.ensure_connection_preloaded(connection_index, cx);
        }
    }

    pub(crate) fn is_connection_refreshing(&self, target_connection_node_id: &str) -> bool {
        self.connections
            .iter()
            .find(|connection| {
                connection_node_id(&connection.profile.id) == target_connection_node_id
            })
            .is_some_and(connection_is_loading)
    }

    pub(crate) fn is_any_refreshing(&self) -> bool {
        self.connections.iter().any(connection_is_loading)
    }

    pub(crate) fn ensure_expanded_loaded(
        &mut self,
        expanded_node_ids: &BTreeSet<String>,
        cx: &mut Context<Self>,
    ) {
        for connection_index in 0..self.connections.len() {
            let connection_id = connection_node_id(&self.connections[connection_index].profile.id);
            if expanded_node_ids.contains(&connection_id) {
                self.ensure_connection_loaded(connection_index, cx);
            }

            let namespace_ids = match &self.connections[connection_index].status {
                LoadState::Loaded(namespaces) => namespaces
                    .iter()
                    .enumerate()
                    .map(|(index, namespace)| {
                        (
                            index,
                            namespace_node_id(
                                &self.connections[connection_index].profile.id,
                                &namespace.info.id,
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };

            for (namespace_index, namespace_id) in namespace_ids {
                if expanded_node_ids.contains(&namespace_id) {
                    self.ensure_namespace_loaded(connection_index, namespace_index, cx);
                }
            }

            let resource_ids = match &self.connections[connection_index].status {
                LoadState::Loaded(namespaces) => namespaces
                    .iter()
                    .enumerate()
                    .flat_map(|(namespace_index, namespace)| match &namespace.resources {
                        LoadState::Loaded(resources) => resources
                            .iter()
                            .enumerate()
                            .map(|(resource_index, resource)| {
                                (
                                    namespace_index,
                                    resource_index,
                                    resource_node_id(
                                        &self.connections[connection_index].profile.id,
                                        &namespace.info.id,
                                        &resource.info.name,
                                    ),
                                )
                            })
                            .collect::<Vec<_>>(),
                        _ => Vec::new(),
                    })
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };

            for (namespace_index, resource_index, resource_id) in resource_ids {
                if expanded_node_ids.contains(&resource_id) {
                    self.ensure_resource_structure_loaded(
                        connection_index,
                        namespace_index,
                        resource_index,
                        cx,
                    );
                }
            }
        }
    }

    pub(crate) fn build_tree(&self, i18n: &I18n) -> SidebarTree {
        SidebarTree::new(
            self.connections
                .iter()
                .map(|connection| self.build_connection_node(connection, i18n))
                .collect(),
        )
    }

    pub(crate) fn is_preload_settled(&self) -> bool {
        self.connections.iter().all(connection_preload_settled)
    }

    fn build_connection_node(&self, connection: &ConnectionEntry, i18n: &I18n) -> SidebarNode {
        let id = connection_node_id(&connection.profile.id);
        SidebarNode {
            id: id.clone(),
            label: connection.profile.name.clone().into(),
            kind: SidebarNodeKind::Connection,
            icon: match connection.profile.provider {
                ResolvedProviderConfig::Sqlite(_) => SidebarIcon::Provider(ProviderIconName::Sqlite),
                ResolvedProviderConfig::Unknown { .. } => SidebarIcon::Lucide(IconName::Database),
            },
            parent_id: None,
            children: match &connection.status {
                LoadState::Unloaded => Vec::new(),
                LoadState::Loading => vec![loading_node(id, i18n)],
                LoadState::Error(message) | LoadState::Unsupported(message) => {
                    vec![error_node(id, message, i18n)]
                }
                LoadState::Loaded(namespaces) => namespaces
                    .iter()
                    .map(|namespace| self.build_namespace_node(&connection.profile.id, namespace, i18n))
                    .collect(),
            },
            trailing_label: None,
            badge_count: match &connection.status {
                LoadState::Loaded(namespaces) => Some(namespaces.len()),
                _ => None,
            },
        }
    }

    fn build_namespace_node(
        &self,
        connection_id: &str,
        namespace: &NamespaceEntry,
        i18n: &I18n,
    ) -> SidebarNode {
        let id = namespace_node_id(connection_id, &namespace.info.id);
        let parent_id = Some(connection_node_id(connection_id));

        let children = match &namespace.resources {
            LoadState::Unloaded => Vec::new(),
            LoadState::Loading => vec![loading_node(id.clone(), i18n)],
            LoadState::Error(message) | LoadState::Unsupported(message) => {
                vec![error_node(id.clone(), message, i18n)]
            }
            LoadState::Loaded(resources) => build_resource_bucket_nodes(connection_id, namespace, resources, i18n),
        };

        SidebarNode {
            id,
            label: namespace.info.name.clone().into(),
            kind: SidebarNodeKind::Namespace,
            icon: SidebarIcon::Lucide(IconName::HardDrive),
            parent_id,
            children,
            trailing_label: None,
            badge_count: None,
        }
    }

    fn ensure_connection_preloaded(&mut self, connection_index: usize, cx: &mut Context<Self>) {
        self.ensure_connection_loaded(connection_index, cx);

        let namespace_count = match &self.connections[connection_index].status {
            LoadState::Loaded(namespaces) => namespaces.len(),
            _ => 0,
        };

        for namespace_index in 0..namespace_count {
            self.ensure_namespace_loaded(connection_index, namespace_index, cx);
        }
    }

    fn ensure_connection_loaded(&mut self, connection_index: usize, cx: &mut Context<Self>) {
        if !matches!(self.connections[connection_index].status, LoadState::Unloaded) {
            return;
        }

        self.connections[connection_index].status = LoadState::Loading;
        let weak = cx.entity().downgrade();
        let profile = self.connections[connection_index].profile.clone();
        let generation = self.connections[connection_index].generation;
        cx.spawn(async move |_, cx| {
            let result = load_namespaces(profile.clone()).await;
            let _ = weak.update(cx, |state, cx| {
                let Some(connection_index) = state
                    .connections
                    .iter_mut()
                    .position(|connection| {
                        connection.profile.id == profile.id && connection.generation == generation
                    })
                else {
                    return;
                };

                state.connections[connection_index].status = match result {
                    Ok(namespaces) => LoadState::Loaded(
                        namespaces
                            .into_iter()
                            .map(|info| NamespaceEntry {
                                info,
                                resources: LoadState::Unloaded,
                            })
                            .collect(),
                    ),
                    Err(LoadFailure::Unsupported(provider)) => LoadState::Unsupported(
                        unsupported_provider_message(&provider, cx.global::<I18n>()),
                    ),
                    Err(LoadFailure::Error(message)) => LoadState::Error(message),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn ensure_namespace_loaded(
        &mut self,
        connection_index: usize,
        namespace_index: usize,
        cx: &mut Context<Self>,
    ) {
        let profile = self.connections[connection_index].profile.clone();
        let generation = self.connections[connection_index].generation;
        let Some(namespace) = self.namespace_mut(connection_index, namespace_index) else {
            return;
        };
        if !matches!(namespace.resources, LoadState::Unloaded) {
            return;
        }

        namespace.resources = LoadState::Loading;
        let weak = cx.entity().downgrade();
        let namespace_id = namespace.info.id.clone();

        cx.spawn(async move |_, cx| {
            let result = load_resources(profile.clone(), namespace_id.clone()).await;
            let _ = weak.update(cx, |state, cx| {
                let Some(connection) = state
                    .connections
                    .iter()
                    .find(|connection| connection.profile.id == profile.id)
                else {
                    return;
                };
                if connection.generation != generation {
                    return;
                }

                let Some(namespace) = state.namespace_mut_by_ids(&profile.id, &namespace_id) else {
                    return;
                };

                namespace.resources = match result {
                    Ok(resources) => LoadState::Loaded(
                        resources
                            .into_iter()
                            .map(|info| ResourceEntry {
                                info,
                                structure: LoadState::Unloaded,
                            })
                            .collect(),
                    ),
                    Err(LoadFailure::Unsupported(provider)) => LoadState::Unsupported(
                        unsupported_provider_message(&provider, cx.global::<I18n>()),
                    ),
                    Err(LoadFailure::Error(message)) => LoadState::Error(message),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn ensure_resource_structure_loaded(
        &mut self,
        connection_index: usize,
        namespace_index: usize,
        resource_index: usize,
        cx: &mut Context<Self>,
    ) {
        let profile = self.connections[connection_index].profile.clone();
        let generation = self.connections[connection_index].generation;
        let Some(namespace) = self.namespace_mut(connection_index, namespace_index) else {
            return;
        };
        let namespace_id = namespace.info.id.clone();
        let Some(resource) = resource_mut(namespace, resource_index) else {
            return;
        };
        if !matches!(resource.structure, LoadState::Unloaded) {
            return;
        }

        resource.structure = LoadState::Loading;
        let resource_name = resource.info.name.clone();
        let resource_ref = resource.info.resource.clone();
        let weak = cx.entity().downgrade();

        cx.spawn(async move |_, cx| {
            let result = load_resource_structure(profile.clone(), resource_ref).await;
            let _ = weak.update(cx, |state, cx| {
                let Some(connection) = state
                    .connections
                    .iter()
                    .find(|connection| connection.profile.id == profile.id)
                else {
                    return;
                };
                if connection.generation != generation {
                    return;
                }

                let Some(resource) = state.resource_mut_by_ids(&profile.id, &namespace_id, &resource_name)
                else {
                    return;
                };

                resource.structure = match result {
                    Ok(structure) => LoadState::Loaded(structure),
                    Err(LoadFailure::Unsupported(provider)) => LoadState::Unsupported(
                        unsupported_provider_message(&provider, cx.global::<I18n>()),
                    ),
                    Err(LoadFailure::Error(message)) => LoadState::Error(message),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn reset_connection(&mut self, connection_index: usize) {
        self.connections[connection_index].generation += 1;
        self.connections[connection_index].status = LoadState::Unloaded;
    }

    fn namespace_mut(
        &mut self,
        connection_index: usize,
        namespace_index: usize,
    ) -> Option<&mut NamespaceEntry> {
        match &mut self.connections[connection_index].status {
            LoadState::Loaded(namespaces) => namespaces.get_mut(namespace_index),
            _ => None,
        }
    }

    fn namespace_mut_by_ids(
        &mut self,
        connection_id: &str,
        namespace_id: &str,
    ) -> Option<&mut NamespaceEntry> {
        let connection = self
            .connections
            .iter_mut()
            .find(|connection| connection.profile.id == connection_id)?;
        match &mut connection.status {
            LoadState::Loaded(namespaces) => namespaces
                .iter_mut()
                .find(|namespace| namespace.info.id == namespace_id),
            _ => None,
        }
    }

    fn resource_mut_by_ids(
        &mut self,
        connection_id: &str,
        namespace_id: &str,
        resource_name: &str,
    ) -> Option<&mut ResourceEntry> {
        let namespace = self.namespace_mut_by_ids(connection_id, namespace_id)?;
        match &mut namespace.resources {
            LoadState::Loaded(resources) => resources
                .iter_mut()
                .find(|resource| resource.info.name == resource_name),
            _ => None,
        }
    }
}

fn build_resource_bucket_nodes(
    connection_id: &str,
    namespace: &NamespaceEntry,
    resources: &[ResourceEntry],
    i18n: &I18n,
) -> Vec<SidebarNode> {
    let tables = resources
        .iter()
        .filter(|resource| classify_resource_kind(resource.info.kind) == SidebarBucketKind::Tables)
        .collect::<Vec<_>>();
    let views = resources
        .iter()
        .filter(|resource| classify_resource_kind(resource.info.kind) == SidebarBucketKind::Views)
        .collect::<Vec<_>>();

    let mut nodes = Vec::new();
    if !tables.is_empty() {
        let table_count = tables.len();
        let id = bucket_node_id(connection_id, &namespace.info.id, SidebarBucketKind::Tables);
        nodes.push(SidebarNode {
            id: id.clone(),
            label: i18n.t("sidebar-group-tables").into(),
            kind: SidebarNodeKind::ResourceBucket,
            icon: SidebarIcon::Lucide(IconName::FolderClosed),
            parent_id: Some(namespace_node_id(connection_id, &namespace.info.id)),
            children: tables
                .iter()
                .map(|resource| build_resource_node(connection_id, &namespace.info.id, resource, i18n))
                .collect(),
            trailing_label: None,
            badge_count: Some(table_count),
        });
    }

    if !views.is_empty() {
        let view_count = views.len();
        let id = bucket_node_id(connection_id, &namespace.info.id, SidebarBucketKind::Views);
        nodes.push(SidebarNode {
            id: id.clone(),
            label: i18n.t("sidebar-group-views").into(),
            kind: SidebarNodeKind::ResourceBucket,
            icon: SidebarIcon::Lucide(IconName::FolderClosed),
            parent_id: Some(namespace_node_id(connection_id, &namespace.info.id)),
            children: views
                .iter()
                .map(|resource| build_resource_node(connection_id, &namespace.info.id, resource, i18n))
                .collect(),
            trailing_label: None,
            badge_count: Some(view_count),
        });
    }

    nodes
}

fn build_resource_child_bucket_nodes(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    structure: &ResourceStructure,
    i18n: &I18n,
) -> Vec<SidebarNode> {
    let mut nodes = Vec::new();

    if !structure.fields.is_empty() {
        let bucket_id = child_bucket_node_id(
            connection_id,
            namespace_id,
            resource_name,
            SidebarBucketKind::Columns,
        );
        nodes.push(SidebarNode {
            id: bucket_id.clone(),
            label: i18n.t("sidebar-group-columns").into(),
            kind: SidebarNodeKind::ResourceChildBucket,
            icon: SidebarIcon::Lucide(IconName::FolderClosed),
            parent_id: Some(resource_node_id(connection_id, namespace_id, resource_name)),
            children: structure
                .fields
                .iter()
                .map(|field| SidebarNode {
                    id: field_node_id(connection_id, namespace_id, resource_name, &field.name),
                    label: field.name.clone().into(),
                    kind: SidebarNodeKind::Field,
                    icon: SidebarIcon::Lucide(IconName::Hash),
                    parent_id: Some(bucket_id.clone()),
                    children: Vec::new(),
                    trailing_label: field_type_label(field).map(Into::into),
                    badge_count: None,
                })
                .collect(),
            trailing_label: None,
            badge_count: Some(structure.fields.len()),
        });
    }

    if !structure.keys.is_empty() {
        let bucket_id = child_bucket_node_id(
            connection_id,
            namespace_id,
            resource_name,
            SidebarBucketKind::Keys,
        );
        nodes.push(SidebarNode {
            id: bucket_id.clone(),
            label: i18n.t("sidebar-group-keys").into(),
            kind: SidebarNodeKind::ResourceChildBucket,
            icon: SidebarIcon::Lucide(IconName::FolderClosed),
            parent_id: Some(resource_node_id(connection_id, namespace_id, resource_name)),
            children: structure
                .keys
                .iter()
                .enumerate()
                .map(|(index, key)| SidebarNode {
                    id: key_node_id(connection_id, namespace_id, resource_name, index),
                    label: key_label(key, index, i18n).into(),
                    kind: SidebarNodeKind::Key,
                    icon: SidebarIcon::Lucide(IconName::KeyRound),
                    parent_id: Some(bucket_id.clone()),
                    children: Vec::new(),
                    trailing_label: None,
                    badge_count: None,
                })
                .collect(),
            trailing_label: None,
            badge_count: Some(structure.keys.len()),
        });
    }

    if !structure.indexes.is_empty() {
        let bucket_id = child_bucket_node_id(
            connection_id,
            namespace_id,
            resource_name,
            SidebarBucketKind::Indexes,
        );
        nodes.push(SidebarNode {
            id: bucket_id.clone(),
            label: i18n.t("sidebar-group-indexes").into(),
            kind: SidebarNodeKind::ResourceChildBucket,
            icon: SidebarIcon::Lucide(IconName::FolderClosed),
            parent_id: Some(resource_node_id(connection_id, namespace_id, resource_name)),
            children: structure
                .indexes
                .iter()
                .enumerate()
                .map(|(index, item)| SidebarNode {
                    id: index_node_id(connection_id, namespace_id, resource_name, index),
                    label: index_label(item, i18n).into(),
                    kind: SidebarNodeKind::Index,
                    icon: SidebarIcon::Lucide(IconName::ListTree),
                    parent_id: Some(bucket_id.clone()),
                    children: Vec::new(),
                    trailing_label: None,
                    badge_count: None,
                })
                .collect(),
            trailing_label: None,
            badge_count: Some(structure.indexes.len()),
        });
    }

    nodes
}

fn build_resource_node(
    connection_id: &str,
    namespace_id: &str,
    resource: &ResourceEntry,
    i18n: &I18n,
) -> SidebarNode {
    let id = resource_node_id(connection_id, namespace_id, &resource.info.name);
    let parent_id = Some(resource_bucket_parent_id(
        connection_id,
        namespace_id,
        classify_resource_kind(resource.info.kind),
    ));
    let icon = match resource.info.kind {
        ResourceKind::View => SidebarIcon::Lucide(IconName::Rows3),
        _ => SidebarIcon::Lucide(IconName::Table),
    };

    let children = match &resource.structure {
        LoadState::Unloaded => Vec::new(),
        LoadState::Loading => vec![loading_node(id.clone(), i18n)],
        LoadState::Error(message) | LoadState::Unsupported(message) => {
            vec![error_node(id.clone(), message, i18n)]
        }
        LoadState::Loaded(structure) => build_resource_child_bucket_nodes(
            connection_id,
            namespace_id,
            &resource.info.name,
            structure,
            i18n,
        ),
    };

    SidebarNode {
        id,
        label: resource.info.name.clone().into(),
        kind: SidebarNodeKind::Resource,
        icon,
        parent_id,
        children,
        trailing_label: None,
        badge_count: None,
    }
}

fn loading_node(parent_id: String, i18n: &I18n) -> SidebarNode {
    SidebarNode {
        id: format!("{parent_id}:loading"),
        label: i18n.t("sidebar-loading").into(),
        kind: SidebarNodeKind::Loading,
        icon: SidebarIcon::Lucide(IconName::SquareTerminal),
        parent_id: Some(parent_id),
        children: Vec::new(),
        trailing_label: None,
        badge_count: None,
    }
}

fn error_node(parent_id: String, message: &str, i18n: &I18n) -> SidebarNode {
    SidebarNode {
        id: format!("{parent_id}:error"),
        label: i18n.t("sidebar-load-error").into(),
        kind: SidebarNodeKind::Error,
        icon: SidebarIcon::Lucide(IconName::SquareTerminal),
        parent_id: Some(parent_id),
        children: Vec::new(),
        trailing_label: Some(message.to_owned().into()),
        badge_count: None,
    }
}

fn resource_bucket_parent_id(
    connection_id: &str,
    namespace_id: &str,
    bucket: SidebarBucketKind,
) -> String {
    bucket_node_id(connection_id, namespace_id, bucket)
}

fn classify_resource_kind(kind: ResourceKind) -> SidebarBucketKind {
    match kind {
        ResourceKind::View => SidebarBucketKind::Views,
        _ => SidebarBucketKind::Tables,
    }
}

fn field_type_label(field: &FieldMeta) -> Option<String> {
    field
        .native_type
        .clone()
        .or_else(|| field.logical_type.map(|logical| format!("{logical:?}").to_ascii_lowercase()))
}

fn key_label(key: &ResourceKeyInfo, index: usize, i18n: &I18n) -> String {
    let name = key.name.clone().unwrap_or_else(|| match key.kind {
        kandb_provider_core::ResourceKeyKind::Primary => {
            i18n.t_with_args("sidebar-key-generated-name", &{
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("index", index + 1);
                args
            })
        }
        _ => i18n.t("sidebar-key-unnamed"),
    });

    format!("{name} ({})", key.columns.join(", "))
}

fn index_label(index: &ResourceIndexInfo, i18n: &I18n) -> String {
    let mut label = format!("{} ({})", index.name, index.columns.join(", "));
    if index.unique {
        label.push(' ');
        label.push_str(&i18n.t("sidebar-index-unique"));
    }
    label
}

fn unsupported_provider_message(provider: &str, i18n: &I18n) -> String {
    i18n.t_with_args("sidebar-provider-unsupported", &{
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("provider", provider);
        args
    })
}

fn connection_node_id(connection_id: &str) -> String {
    format!("connection:{connection_id}")
}

fn namespace_node_id(connection_id: &str, namespace_id: &str) -> String {
    format!("namespace:{connection_id}:{namespace_id}")
}

fn bucket_node_id(connection_id: &str, namespace_id: &str, bucket: SidebarBucketKind) -> String {
    format!("bucket:{connection_id}:{namespace_id}:{}", bucket_slug(bucket))
}

fn child_bucket_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    bucket: SidebarBucketKind,
) -> String {
    format!(
        "child-bucket:{connection_id}:{namespace_id}:{resource_name}:{}",
        bucket_slug(bucket)
    )
}

fn resource_node_id(connection_id: &str, namespace_id: &str, resource_name: &str) -> String {
    format!("resource:{connection_id}:{namespace_id}:{resource_name}")
}

fn field_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    field_name: &str,
) -> String {
    format!("field:{connection_id}:{namespace_id}:{resource_name}:{field_name}")
}

fn key_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    index: usize,
) -> String {
    format!("key:{connection_id}:{namespace_id}:{resource_name}:{index}")
}

fn index_node_id(
    connection_id: &str,
    namespace_id: &str,
    resource_name: &str,
    index: usize,
) -> String {
    format!("index:{connection_id}:{namespace_id}:{resource_name}:{index}")
}

fn bucket_slug(bucket: SidebarBucketKind) -> &'static str {
    match bucket {
        SidebarBucketKind::Tables => "tables",
        SidebarBucketKind::Views => "views",
        SidebarBucketKind::Columns => "columns",
        SidebarBucketKind::Keys => "keys",
        SidebarBucketKind::Indexes => "indexes",
    }
}

enum LoadFailure {
    Unsupported(String),
    Error(String),
}

async fn load_namespaces(profile: ResolvedConnectionProfile) -> Result<Vec<NamespaceInfo>, LoadFailure> {
    let connection = connect_profile(&profile).await?;
    connection
        .list_namespaces()
        .await
        .map_err(|error| LoadFailure::Error(error.to_string()))
}

async fn load_resources(
    profile: ResolvedConnectionProfile,
    namespace_id: String,
) -> Result<Vec<ResourceInfo>, LoadFailure> {
    let connection = connect_profile(&profile).await?;
    connection
        .list_resources(
            &namespace_id,
            kandb_provider_core::ListResourcesRequest::default(),
        )
        .await
        .map(|page| page.items)
        .map_err(|error| LoadFailure::Error(error.to_string()))
}

async fn load_resource_structure(
    profile: ResolvedConnectionProfile,
    resource: ResourceRef,
) -> Result<ResourceStructure, LoadFailure> {
    let connection = connect_profile(&profile).await?;
    let fields = connection
        .list_fields(&resource)
        .await
        .map_err(|error| LoadFailure::Error(error.to_string()))?
        .unwrap_or_default();

    let keys = if let Some(introspector) = connection.resource_structure_introspector() {
        introspector
            .list_keys(&resource)
            .await
            .map_err(|error| LoadFailure::Error(error.to_string()))?
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let indexes = if let Some(introspector) = connection.resource_structure_introspector() {
        introspector
            .list_indexes(&resource)
            .await
            .map_err(|error| LoadFailure::Error(error.to_string()))?
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(ResourceStructure {
        fields,
        keys,
        indexes,
    })
}

async fn connect_profile(
    profile: &ResolvedConnectionProfile,
) -> Result<Box<dyn Connection>, LoadFailure> {
    match &profile.provider {
        ResolvedProviderConfig::Sqlite(config) => SqliteProvider
            .connect(config.clone())
            .await
            .map(|connection| Box::new(connection) as Box<dyn Connection>)
            .map_err(|error| LoadFailure::Error(error.to_string())),
        ResolvedProviderConfig::Unknown { provider, .. } => {
            Err(LoadFailure::Unsupported(provider.clone()))
        }
    }
}

fn resource_mut(namespace: &mut NamespaceEntry, resource_index: usize) -> Option<&mut ResourceEntry> {
    match &mut namespace.resources {
        LoadState::Loaded(resources) => resources.get_mut(resource_index),
        _ => None,
    }
}

fn connection_is_loading(connection: &ConnectionEntry) -> bool {
    match &connection.status {
        LoadState::Loading => true,
        LoadState::Loaded(namespaces) => namespaces.iter().any(namespace_is_loading),
        LoadState::Unloaded | LoadState::Error(_) | LoadState::Unsupported(_) => false,
    }
}

fn namespace_is_loading(namespace: &NamespaceEntry) -> bool {
    match &namespace.resources {
        LoadState::Loading => true,
        LoadState::Loaded(resources) => resources.iter().any(resource_is_loading),
        LoadState::Unloaded | LoadState::Error(_) | LoadState::Unsupported(_) => false,
    }
}

fn resource_is_loading(resource: &ResourceEntry) -> bool {
    matches!(resource.structure, LoadState::Loading)
}

fn connection_preload_settled(connection: &ConnectionEntry) -> bool {
    match &connection.status {
        LoadState::Loaded(namespaces) => namespaces.iter().all(namespace_preload_settled),
        LoadState::Error(_) | LoadState::Unsupported(_) => true,
        LoadState::Unloaded | LoadState::Loading => false,
    }
}

fn namespace_preload_settled(namespace: &NamespaceEntry) -> bool {
    !matches!(namespace.resources, LoadState::Unloaded | LoadState::Loading)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_paths::AppPaths,
        config::{AppConfigFile, LoadedAppConfig, StoredConnectionProfile},
    };
    use kandb_provider_sqlite::{SqliteConfig, SqliteLocation};
    use std::path::PathBuf;

    fn sample_state() -> SidebarState {
        SidebarState {
            connections: vec![ConnectionEntry {
                profile: ResolvedConnectionProfile {
                    id: "local-main".into(),
                    name: "Local Main".into(),
                    provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                        location: SqliteLocation::Memory,
                        read_only: false,
                        create_if_missing: true,
                    }),
                },
                generation: 0,
                status: LoadState::Loaded(vec![NamespaceEntry {
                    info: NamespaceInfo {
                        id: "main".into(),
                        name: "main".into(),
                        kind: kandb_provider_core::NamespaceKind::Database,
                        parent_id: None,
                    },
                    resources: LoadState::Loaded(vec![
                        ResourceEntry {
                            info: ResourceInfo {
                                resource: ResourceRef {
                                    namespace_id: "main".into(),
                                    resource_id: "sqlite_schema".into(),
                                },
                                name: "sqlite_schema".into(),
                                kind: ResourceKind::Table,
                            },
                            structure: LoadState::Loaded(ResourceStructure {
                                fields: vec![FieldMeta {
                                    ordinal: Some(0),
                                    name: "type".into(),
                                    logical_type: Some(kandb_provider_core::LogicalType::Text),
                                    native_type: Some("TEXT".into()),
                                    nullable: Some(false),
                                    default_value_sql: None,
                                    primary_key_ordinal: None,
                                    hidden: Some(false),
                                }],
                                keys: Vec::new(),
                                indexes: Vec::new(),
                            }),
                        },
                        ResourceEntry {
                            info: ResourceInfo {
                                resource: ResourceRef {
                                    namespace_id: "main".into(),
                                    resource_id: "user_names".into(),
                                },
                                name: "user_names".into(),
                                kind: ResourceKind::View,
                            },
                            structure: LoadState::Unloaded,
                        },
                    ]),
                }]),
            }],
        }
    }

    #[test]
    fn sqlite_schema_stays_in_tables_bucket() {
        let tree = sample_state().build_tree(&I18n::english_for_test());
        let visible = tree.visible_nodes(&tree.default_expanded_node_ids(None));

        assert!(visible.iter().any(|node| node.id == "resource:local-main:main:sqlite_schema"));
        assert!(!visible.iter().any(|node| node.label == "System"));
    }

    #[test]
    fn default_expansion_reaches_first_resource_bucket() {
        let tree = sample_state().build_tree(&I18n::english_for_test());

        assert_eq!(
            tree.default_expanded_node_ids(None),
            BTreeSet::from([
                "connection:local-main".to_string(),
                "namespace:local-main:main".to_string(),
                "bucket:local-main:main:tables".to_string(),
            ])
        );
    }

    #[test]
    fn from_config_preserves_connection_roots() {
        let config = LoadedAppConfig {
            paths: AppPaths::from_roots(PathBuf::from("/tmp/config"), PathBuf::from("/tmp/data")),
            file: AppConfigFile {
                version: 1,
                default_connection_id: Some("local-main".into()),
                connections: vec![StoredConnectionProfile {
                    id: "local-main".into(),
                    name: "Local Main".into(),
                    provider: "sqlite".into(),
                    config: toml::Table::new(),
                }],
            },
            resolved_connections: vec![ResolvedConnectionProfile {
                id: "local-main".into(),
                name: "Local Main".into(),
                provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                    location: SqliteLocation::Memory,
                    read_only: false,
                    create_if_missing: true,
                }),
            }],
        };

        let state = SidebarState::from_config(&config);
        assert_eq!(state.connections.len(), 1);
        assert_eq!(state.connections[0].generation, 0);
        assert!(matches!(state.connections[0].status, LoadState::Unloaded));
    }

    #[test]
    fn preload_is_not_settled_while_namespace_resources_are_loading() {
        let state = SidebarState {
            connections: vec![ConnectionEntry {
                profile: ResolvedConnectionProfile {
                    id: "local-main".into(),
                    name: "Local Main".into(),
                    provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                        location: SqliteLocation::Memory,
                        read_only: false,
                        create_if_missing: true,
                    }),
                },
                generation: 0,
                status: LoadState::Loaded(vec![NamespaceEntry {
                    info: NamespaceInfo {
                        id: "main".into(),
                        name: "main".into(),
                        kind: kandb_provider_core::NamespaceKind::Database,
                        parent_id: None,
                    },
                    resources: LoadState::Loading,
                }]),
            }],
        };

        assert!(!state.is_preload_settled());
    }

    #[test]
    fn preload_is_settled_once_namespace_resources_are_materialized() {
        assert!(sample_state().is_preload_settled());
    }
}
