use async_compat::Compat;
use async_lock::Mutex;
use async_trait::async_trait;
use kandb_i18n::Translator;
use kandb_provider_core::{
    Connection, ConnectionFormSchema, FormField, FormFieldKind, FormSelectOption, IconToken,
    ProviderError, ProviderErrorKind, ProviderPlugin, Result, SidebarTree, TreeChildren, TreeNode,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    Connection as _, Row,
    sqlite::{SqliteConnectOptions, SqliteConnection, SqliteJournalMode, SqliteRow},
};
use std::{future::Future, path::PathBuf, pin::Pin, str::FromStr};

const SQLITE_KIND: &str = "sqlite";

pub struct SqlitePlugin;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub location: SqliteLocation,
    pub read_only: bool,
    pub create_if_missing: bool,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            location: SqliteLocation::Memory,
            read_only: false,
            create_if_missing: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqliteLocation {
    Memory,
    Path(PathBuf),
    Uri(String),
}

pub struct SqliteConnectionHandle {
    connection: Mutex<SqliteConnection>,
}

#[async_trait]
impl ProviderPlugin for SqlitePlugin {
    type Config = SqliteConfig;
    type Connection = SqliteConnectionHandle;

    fn kind(&self) -> &'static str {
        SQLITE_KIND
    }

    fn display_name(&self) -> &'static str {
        "SQLite"
    }

    fn connection_form(&self, locale: &str) -> ConnectionFormSchema {
        let i18n = Translator::for_locale_tag(locale);

        ConnectionFormSchema {
            title: i18n.t("provider-sqlite-connection-title"),
            fields: vec![
                FormField {
                    key: "location_kind".into(),
                    label: i18n.t("provider-sqlite-field-location"),
                    kind: FormFieldKind::Select,
                    required: true,
                    help_text: Some(i18n.t("provider-sqlite-field-location-help")),
                    placeholder: None,
                    options: vec![
                        FormSelectOption {
                            value: "memory".into(),
                            label: i18n.t("provider-sqlite-option-memory"),
                        },
                        FormSelectOption {
                            value: "path".into(),
                            label: i18n.t("provider-sqlite-option-path"),
                        },
                        FormSelectOption {
                            value: "uri".into(),
                            label: i18n.t("provider-sqlite-option-uri"),
                        },
                    ],
                },
                FormField {
                    key: "location_value".into(),
                    label: i18n.t("provider-sqlite-field-location-value"),
                    kind: FormFieldKind::Path,
                    required: false,
                    help_text: Some(i18n.t("provider-sqlite-field-location-value-help")),
                    placeholder: Some("file:app.db?mode=rwc".into()),
                    options: Vec::new(),
                },
                FormField {
                    key: "read_only".into(),
                    label: i18n.t("provider-sqlite-field-read-only"),
                    kind: FormFieldKind::Checkbox,
                    required: false,
                    help_text: None,
                    placeholder: None,
                    options: Vec::new(),
                },
                FormField {
                    key: "create_if_missing".into(),
                    label: i18n.t("provider-sqlite-field-create-if-missing"),
                    kind: FormFieldKind::Checkbox,
                    required: false,
                    help_text: None,
                    placeholder: None,
                    options: Vec::new(),
                },
            ],
        }
    }

    async fn connect(&self, config: Self::Config) -> Result<Self::Connection> {
        let connection = connect_with_config(&config).await?;
        Ok(SqliteConnectionHandle {
            connection: Mutex::new(connection),
        })
    }

    async fn test_connection(&self, config: &Self::Config) -> Result<()> {
        let mut connection = connect_with_config(config).await?;
        run_sqlx(async {
            sqlx::query("SELECT 1").execute(&mut connection).await?;
            Ok::<_, sqlx::Error>(())
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Ping, err.to_string()))
    }
}

#[async_trait]
impl Connection for SqliteConnectionHandle {
    fn kind(&self) -> &'static str {
        SQLITE_KIND
    }

    async fn ping(&self) -> Result<()> {
        self.with_connection(|connection| {
            Box::pin(async move {
                sqlx::query("SELECT 1").execute(connection).await?;
                Ok(())
            })
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Ping, err.to_string()))
    }

    async fn load_sidebar(&self, locale: &str) -> Result<SidebarTree> {
        let i18n = Translator::for_locale_tag(locale);
        self.with_connection(|connection| {
            Box::pin(async move {
                let namespaces = load_namespaces(connection).await?;
                let mut roots = Vec::with_capacity(namespaces.len());
                for namespace in namespaces {
                    roots.push(load_namespace_node(connection, &namespace, &i18n).await?);
                }
                Ok(SidebarTree { roots })
            })
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Metadata, err.to_string()))
    }
}

impl SqliteConnectionHandle {
    async fn with_connection<F, T>(&self, operation: F) -> std::result::Result<T, sqlx::Error>
    where
        F: for<'a> FnOnce(
                &'a mut SqliteConnection,
            ) -> Pin<
                Box<dyn Future<Output = std::result::Result<T, sqlx::Error>> + Send + 'a>,
            > + Send,
        T: Send,
    {
        let mut connection = self.connection.lock().await;
        run_sqlx(operation(&mut connection)).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamespaceMeta {
    id: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceMeta {
    name: String,
    kind: ResourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Table,
    View,
    VirtualTable,
    ShadowTable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldMeta {
    name: String,
    type_label: Option<String>,
    primary_key_ordinal: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceKeyMeta {
    name: Option<String>,
    kind: KeyKind,
    columns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyKind {
    Primary,
    Unique,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceIndexMeta {
    name: String,
    columns: Vec<String>,
    unique: bool,
}

#[derive(Debug)]
struct SqliteIndexMeta {
    name: Option<String>,
    origin: String,
    unique: bool,
    columns: Vec<String>,
}

#[derive(Debug)]
struct SqliteIndexColumnName {
    seqno: i64,
    name: String,
}

async fn load_namespaces(
    connection: &mut SqliteConnection,
) -> std::result::Result<Vec<NamespaceMeta>, sqlx::Error> {
    let rows = sqlx::query("PRAGMA database_list")
        .fetch_all(connection)
        .await?;
    let mut namespaces = Vec::with_capacity(rows.len());

    for row in rows {
        let name: String = row.try_get("name")?;
        namespaces.push(NamespaceMeta {
            id: name.clone(),
            name,
        });
    }

    Ok(namespaces)
}

async fn load_namespace_node(
    connection: &mut SqliteConnection,
    namespace: &NamespaceMeta,
    i18n: &Translator,
) -> std::result::Result<TreeNode, sqlx::Error> {
    let resources = load_resources(connection, &namespace.id).await?;
    let table_nodes = load_resource_nodes(connection, namespace, &resources, i18n, false).await?;
    let view_nodes = load_resource_nodes(connection, namespace, &resources, i18n, true).await?;

    let mut children = Vec::new();
    if !table_nodes.is_empty() {
        children.push(TreeNode {
            id: encode_tree_id(&SqliteTreeNodeId::Bucket {
                namespace_id: namespace.id.clone(),
                bucket: "tables".into(),
            }),
            label: i18n.t("provider-sqlite-sidebar-group-tables"),
            icon: IconToken::Folder,
            children: TreeChildren::Branch(table_nodes),
        });
    }
    if !view_nodes.is_empty() {
        children.push(TreeNode {
            id: encode_tree_id(&SqliteTreeNodeId::Bucket {
                namespace_id: namespace.id.clone(),
                bucket: "views".into(),
            }),
            label: i18n.t("provider-sqlite-sidebar-group-views"),
            icon: IconToken::Folder,
            children: TreeChildren::Branch(view_nodes),
        });
    }

    Ok(TreeNode {
        id: encode_tree_id(&SqliteTreeNodeId::Namespace {
            namespace_id: namespace.id.clone(),
        }),
        label: namespace.name.clone(),
        icon: IconToken::HardDrive,
        children: TreeChildren::Branch(children),
    })
}

async fn load_resources(
    connection: &mut SqliteConnection,
    namespace_id: &str,
) -> std::result::Result<Vec<ResourceMeta>, sqlx::Error> {
    let pragma_sql = format!("PRAGMA {}.table_list", quote_identifier(namespace_id));
    let rows = sqlx::query(&pragma_sql).fetch_all(connection).await?;
    let mut resources = Vec::with_capacity(rows.len());

    for row in rows {
        let name: String = row.try_get("name")?;
        resources.push(ResourceMeta {
            name,
            kind: map_resource_kind(&row)?,
        });
    }

    resources.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(resources)
}

async fn load_resource_nodes(
    connection: &mut SqliteConnection,
    namespace: &NamespaceMeta,
    resources: &[ResourceMeta],
    i18n: &Translator,
    views_only: bool,
) -> std::result::Result<Vec<TreeNode>, sqlx::Error> {
    let filtered = resources
        .iter()
        .filter(|resource| matches!(resource.kind, ResourceKind::View) == views_only)
        .collect::<Vec<_>>();
    let mut nodes = Vec::with_capacity(filtered.len());

    for resource in filtered {
        nodes.push(load_resource_node(connection, namespace, resource, i18n).await?);
    }

    Ok(nodes)
}

async fn load_resource_node(
    connection: &mut SqliteConnection,
    namespace: &NamespaceMeta,
    resource: &ResourceMeta,
    i18n: &Translator,
) -> std::result::Result<TreeNode, sqlx::Error> {
    let fields = load_fields(connection, &namespace.id, &resource.name).await?;
    let keys = load_keys(connection, &namespace.id, &resource.name, &fields).await?;
    let indexes = load_indexes(connection, &namespace.id, &resource.name).await?;

    let mut children = Vec::new();

    if !fields.is_empty() {
        children.push(TreeNode {
            id: encode_tree_id(&SqliteTreeNodeId::Bucket {
                namespace_id: namespace.id.clone(),
                bucket: format!("columns:{}", resource.name),
            }),
            label: i18n.t("provider-sqlite-sidebar-group-columns"),
            icon: IconToken::Folder,
            children: TreeChildren::Branch(
                fields
                    .into_iter()
                    .map(|field| TreeNode {
                        id: encode_tree_id(&SqliteTreeNodeId::Field {
                            namespace_id: namespace.id.clone(),
                            resource_name: resource.name.clone(),
                            field_name: field.name.clone(),
                        }),
                        label: field.type_label.map_or(field.name.clone(), |type_label| {
                            format!("{} ({type_label})", field.name)
                        }),
                        icon: IconToken::Column,
                        children: TreeChildren::Leaf,
                    })
                    .collect(),
            ),
        });
    }

    if !keys.is_empty() {
        children.push(TreeNode {
            id: encode_tree_id(&SqliteTreeNodeId::Bucket {
                namespace_id: namespace.id.clone(),
                bucket: format!("keys:{}", resource.name),
            }),
            label: i18n.t("provider-sqlite-sidebar-group-keys"),
            icon: IconToken::Folder,
            children: TreeChildren::Branch(
                keys.into_iter()
                    .enumerate()
                    .map(|(index, key)| TreeNode {
                        id: encode_tree_id(&SqliteTreeNodeId::Key {
                            namespace_id: namespace.id.clone(),
                            resource_name: resource.name.clone(),
                            key_signature: key_signature(index, &key),
                        }),
                        label: key_label(i18n, index, &key),
                        icon: IconToken::Key,
                        children: TreeChildren::Leaf,
                    })
                    .collect(),
            ),
        });
    }

    if !indexes.is_empty() {
        children.push(TreeNode {
            id: encode_tree_id(&SqliteTreeNodeId::Bucket {
                namespace_id: namespace.id.clone(),
                bucket: format!("indexes:{}", resource.name),
            }),
            label: i18n.t("provider-sqlite-sidebar-group-indexes"),
            icon: IconToken::Folder,
            children: TreeChildren::Branch(
                indexes
                    .into_iter()
                    .map(|index| TreeNode {
                        id: encode_tree_id(&SqliteTreeNodeId::Index {
                            namespace_id: namespace.id.clone(),
                            resource_name: resource.name.clone(),
                            index_name: index.name.clone(),
                        }),
                        label: index_label(i18n, &index),
                        icon: IconToken::Index,
                        children: TreeChildren::Leaf,
                    })
                    .collect(),
            ),
        });
    }

    Ok(TreeNode {
        id: encode_tree_id(&SqliteTreeNodeId::Resource {
            namespace_id: namespace.id.clone(),
            resource_name: resource.name.clone(),
        }),
        label: resource.name.clone(),
        icon: if matches!(resource.kind, ResourceKind::View) {
            IconToken::View
        } else {
            IconToken::Table
        },
        children: TreeChildren::Branch(children),
    })
}

async fn load_fields(
    connection: &mut SqliteConnection,
    namespace_id: &str,
    resource_name: &str,
) -> std::result::Result<Vec<FieldMeta>, sqlx::Error> {
    let schema = quote_identifier(namespace_id);
    let table = quote_pragma_argument(resource_name);
    let pragma_sql = format!("PRAGMA {schema}.table_xinfo({table})");
    let rows = sqlx::query(&pragma_sql).fetch_all(connection).await?;

    rows.into_iter().map(map_field_meta).collect()
}

async fn load_keys(
    connection: &mut SqliteConnection,
    namespace_id: &str,
    resource_name: &str,
    fields: &[FieldMeta],
) -> std::result::Result<Vec<ResourceKeyMeta>, sqlx::Error> {
    let indexes = load_resource_indexes(connection, namespace_id, resource_name).await?;

    let mut keys = indexes
        .into_iter()
        .filter_map(|index| match index.origin.as_str() {
            "pk" => Some(ResourceKeyMeta {
                name: normalize_key_name(index.name.as_deref()),
                kind: KeyKind::Primary,
                columns: index.columns,
            }),
            "u" => Some(ResourceKeyMeta {
                name: normalize_key_name(index.name.as_deref()),
                kind: KeyKind::Unique,
                columns: index.columns,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    if !keys.iter().any(|key| key.kind == KeyKind::Primary)
        && let Some(primary_key) = load_primary_key_from_fields(fields)
    {
        keys.insert(0, primary_key);
    }

    Ok(keys)
}

async fn load_indexes(
    connection: &mut SqliteConnection,
    namespace_id: &str,
    resource_name: &str,
) -> std::result::Result<Vec<ResourceIndexMeta>, sqlx::Error> {
    Ok(
        load_resource_indexes(connection, namespace_id, resource_name)
            .await?
            .into_iter()
            .map(|index| ResourceIndexMeta {
                name: index.name.unwrap_or_else(|| "<unnamed>".to_string()),
                columns: index.columns,
                unique: index.unique,
            })
            .collect(),
    )
}

fn load_primary_key_from_fields(fields: &[FieldMeta]) -> Option<ResourceKeyMeta> {
    let mut primary_columns = fields
        .iter()
        .filter_map(|field| {
            field
                .primary_key_ordinal
                .map(|ordinal| (ordinal, field.name.clone()))
        })
        .collect::<Vec<_>>();

    if primary_columns.is_empty() {
        return None;
    }

    primary_columns.sort_by_key(|(ordinal, _)| *ordinal);

    Some(ResourceKeyMeta {
        name: None,
        kind: KeyKind::Primary,
        columns: primary_columns.into_iter().map(|(_, name)| name).collect(),
    })
}

async fn load_resource_indexes(
    connection: &mut SqliteConnection,
    namespace_id: &str,
    resource_name: &str,
) -> std::result::Result<Vec<SqliteIndexMeta>, sqlx::Error> {
    let schema = quote_identifier(namespace_id);
    let table = quote_pragma_argument(resource_name);
    let index_list_sql = format!("PRAGMA {schema}.index_list({table})");
    let rows = sqlx::query(&index_list_sql)
        .fetch_all(&mut *connection)
        .await?;
    let mut indexes = Vec::with_capacity(rows.len());

    for row in rows {
        let name = row.try_get::<Option<String>, _>("name")?;
        let origin = row.try_get::<String, _>("origin")?;
        let unique = row.try_get::<i64, _>("unique")? != 0;
        let Some(index_name) = name.clone() else {
            continue;
        };

        let index_info_sql = format!(
            "PRAGMA {schema}.index_xinfo({})",
            quote_pragma_argument(&index_name)
        );
        let index_rows = sqlx::query(&index_info_sql)
            .fetch_all(&mut *connection)
            .await?;
        let mut columns = index_rows
            .into_iter()
            .filter_map(map_index_column_name)
            .collect::<Vec<_>>();
        columns.sort_by_key(|entry| entry.seqno);

        indexes.push(SqliteIndexMeta {
            name,
            origin,
            unique,
            columns: columns.into_iter().map(|entry| entry.name).collect(),
        });
    }

    Ok(indexes)
}

async fn connect_with_config(config: &SqliteConfig) -> Result<SqliteConnection> {
    let options = connect_options(config)?;
    run_sqlx(SqliteConnection::connect_with(&options))
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Connect, err.to_string()))
}

fn connect_options(config: &SqliteConfig) -> Result<SqliteConnectOptions> {
    let mut options = match &config.location {
        SqliteLocation::Memory => SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|err| ProviderError::invalid_config(err.to_string()))?,
        SqliteLocation::Path(path) => SqliteConnectOptions::new().filename(path),
        SqliteLocation::Uri(uri) => SqliteConnectOptions::from_str(uri)
            .map_err(|err| ProviderError::invalid_config(err.to_string()))?,
    };

    options = options
        .create_if_missing(config.create_if_missing)
        .read_only(config.read_only)
        .foreign_keys(true);

    if !config.read_only {
        options = options.journal_mode(SqliteJournalMode::Wal);
    }

    Ok(options)
}

async fn run_sqlx<T>(future: impl Future<Output = T>) -> T {
    Compat::new(future).await
}

fn map_resource_kind(row: &SqliteRow) -> std::result::Result<ResourceKind, sqlx::Error> {
    let kind: String = row.try_get("type")?;
    Ok(match kind.as_str() {
        "table" => ResourceKind::Table,
        "view" => ResourceKind::View,
        "virtual" => ResourceKind::VirtualTable,
        "shadow" => ResourceKind::ShadowTable,
        _ => ResourceKind::Unknown,
    })
}

fn map_field_meta(row: SqliteRow) -> std::result::Result<FieldMeta, sqlx::Error> {
    let name: String = row.try_get("name")?;
    let native_type = row.try_get::<Option<String>, _>("type")?;
    let primary_key_ordinal = row.try_get::<i64, _>("pk")?;

    Ok(FieldMeta {
        name,
        type_label: native_type,
        primary_key_ordinal: u32::try_from(primary_key_ordinal)
            .ok()
            .filter(|value| *value > 0),
    })
}

fn map_index_column_name(row: SqliteRow) -> Option<SqliteIndexColumnName> {
    let key = row.try_get::<i64, _>("key").ok()?;
    if key == 0 {
        return None;
    }

    let cid = row.try_get::<i64, _>("cid").ok()?;
    if cid < 0 {
        return None;
    }

    let seqno = row.try_get::<i64, _>("seqno").ok()?;
    let name = row.try_get::<Option<String>, _>("name").ok()??;

    Some(SqliteIndexColumnName { seqno, name })
}

fn normalize_key_name(name: Option<&str>) -> Option<String> {
    match name {
        Some(name) if name.starts_with("sqlite_autoindex_") => None,
        Some(name) => Some(name.to_string()),
        None => None,
    }
}

fn key_label(i18n: &Translator, index: usize, key: &ResourceKeyMeta) -> String {
    let name = key.name.clone().unwrap_or_else(|| match key.kind {
        KeyKind::Primary => match i18n
            .locale_tag()
            .split('-')
            .next()
            .unwrap_or(i18n.locale_tag())
        {
            "zh" => format!("键 #{}", index + 1),
            _ => format!("key #{}", index + 1),
        },
        KeyKind::Unique => i18n.t("provider-sqlite-key-unnamed"),
    });

    format!("{name} ({})", key.columns.join(", "))
}

fn index_label(i18n: &Translator, index: &ResourceIndexMeta) -> String {
    let mut label = format!("{} ({})", index.name, index.columns.join(", "));
    if index.unique {
        label.push(' ');
        label.push_str(&i18n.t("provider-sqlite-key-unique"));
    }
    label
}

fn key_signature(index: usize, key: &ResourceKeyMeta) -> String {
    let kind = match key.kind {
        KeyKind::Primary => "primary",
        KeyKind::Unique => "unique",
    };
    format!(
        "{kind}|{index}|{}|{}",
        key.name.as_deref().unwrap_or(""),
        key.columns.join("\u{1f}")
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum SqliteTreeNodeId {
    Namespace {
        namespace_id: String,
    },
    Bucket {
        namespace_id: String,
        bucket: String,
    },
    Resource {
        namespace_id: String,
        resource_name: String,
    },
    Field {
        namespace_id: String,
        resource_name: String,
        field_name: String,
    },
    Key {
        namespace_id: String,
        resource_name: String,
        key_signature: String,
    },
    Index {
        namespace_id: String,
        resource_name: String,
        index_name: String,
    },
}

fn encode_tree_id(node_id: &SqliteTreeNodeId) -> String {
    let encoded = serde_json::to_vec(node_id).expect("sqlite tree id must serialize");
    format!("sqlite:{}", hex_encode(&encoded))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(hex_char(byte >> 4));
        out.push(hex_char(byte & 0x0f));
    }
    out
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("hex nibble out of range"),
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_pragma_argument(identifier: &str) -> String {
    format!("'{}'", identifier.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn connection_form_is_localized() {
        let plugin = SqlitePlugin;
        let form = plugin.connection_form("zh-CN");

        assert_eq!(form.title, "SQLite 连接");
        assert_eq!(form.fields[0].label, "位置");
    }

    #[test]
    fn sqlite_sidebar_tree_contains_namespace_resource_and_structure_groups() {
        block_on(async {
            let plugin = SqlitePlugin;
            let connection = plugin.connect(SqliteConfig::default()).await.unwrap();

            connection
                .with_connection(|connection| {
                    Box::pin(async move {
                        sqlx::query(
                            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT UNIQUE)",
                        )
                        .execute(&mut *connection)
                        .await?;
                        sqlx::query("CREATE VIEW user_names AS SELECT name FROM users")
                            .execute(&mut *connection)
                            .await?;
                        Ok(())
                    })
                })
                .await
                .unwrap();

            let tree = connection.load_sidebar("en-US").await.unwrap();
            let namespace = &tree.roots[0];
            let namespace_children = match &namespace.children {
                TreeChildren::Branch(children) => children,
                TreeChildren::Leaf => panic!("namespace should be a branch"),
            };

            assert!(namespace_children.iter().any(|node| node.label == "Tables"));
            assert!(namespace_children.iter().any(|node| node.label == "Views"));
        });
    }

    #[test]
    fn leaf_and_empty_branch_semantics_are_used_explicitly() {
        let leaf = TreeNode {
            id: "leaf".into(),
            label: "leaf".into(),
            icon: IconToken::Column,
            children: TreeChildren::Leaf,
        };
        let branch = TreeNode {
            id: "branch".into(),
            label: "branch".into(),
            icon: IconToken::Folder,
            children: TreeChildren::Branch(Vec::new()),
        };

        assert_ne!(leaf.children, branch.children);
    }
}
