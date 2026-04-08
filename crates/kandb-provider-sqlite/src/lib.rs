use async_compat::Compat;
use async_lock::Mutex;
use async_trait::async_trait;
use kandb_provider_core::{
    Connection, FieldMeta, ListResourcesPage, ListResourcesRequest, LogicalType, NamespaceInfo,
    NamespaceKind, ProviderError, ProviderErrorKind, ProviderFactory, QueryResult, QueryRow,
    ReadRequest, ResourceIndexInfo, ResourceInfo, ResourceKeyInfo, ResourceKeyKind,
    ResourceKind, ResourceReader, ResourceRef, ResourceStructureIntrospector, Result,
    TextQueryBuilder, TextQueryExecutor, Value,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    Column, Connection as _, Row, TypeInfo, ValueRef,
    sqlite::{SqliteConnectOptions, SqliteConnection, SqliteJournalMode, SqliteRow},
};
use std::{future::Future, path::PathBuf, pin::Pin, str::FromStr};

const SQLITE_KIND: &str = "sqlite";

pub struct SqliteProvider;

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
    config: SqliteConfig,
    connection: Mutex<SqliteConnection>,
}

#[async_trait]
impl ProviderFactory for SqliteProvider {
    type Config = SqliteConfig;
    type Connection = SqliteConnectionHandle;

    fn kind(&self) -> &'static str {
        SQLITE_KIND
    }

    fn display_name(&self) -> &'static str {
        "SQLite"
    }

    async fn connect(&self, config: Self::Config) -> Result<Self::Connection> {
        let connection = connect_with_config(&config).await?;
        Ok(SqliteConnectionHandle {
            config,
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

    async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>> {
        self.with_connection(|connection| {
            Box::pin(async move {
                let rows = sqlx::query("PRAGMA database_list")
                    .fetch_all(connection)
                    .await?;
                let mut namespaces = Vec::with_capacity(rows.len());

                for row in rows {
                    let name: String = row.try_get("name")?;
                    namespaces.push(NamespaceInfo {
                        id: name.clone(),
                        name,
                        kind: NamespaceKind::Database,
                        parent_id: None,
                    });
                }

                Ok(namespaces)
            })
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Metadata, err.to_string()))
    }

    async fn list_resources(
        &self,
        namespace_id: &str,
        request: ListResourcesRequest,
    ) -> Result<ListResourcesPage> {
        let namespace_id = namespace_id.to_string();
        let pragma_sql = format!("PRAGMA {}.table_list", quote_identifier(&namespace_id));
        let pattern = request.pattern.clone();
        let offset = parse_cursor_offset(request.cursor.as_deref())?;
        let limit = request.limit.map(|value| value as usize);

        self.with_connection(|connection| {
            Box::pin(async move {
                let rows = sqlx::query(&pragma_sql).fetch_all(connection).await?;
                let mut resources = Vec::new();

                for row in rows {
                    let name: String = row.try_get("name")?;
                    if let Some(pattern) = pattern.as_deref()
                        && !like_matches(pattern, &name)
                    {
                        continue;
                    }

                    let kind = map_resource_kind(&row)?;
                    resources.push(ResourceInfo {
                        resource: ResourceRef {
                            namespace_id: namespace_id.clone(),
                            resource_id: name.clone(),
                        },
                        name,
                        kind,
                    });
                }

                resources.sort_by(|left, right| left.name.cmp(&right.name));

                let page_items = if let Some(limit) = limit {
                    resources
                        .iter()
                        .skip(offset)
                        .take(limit)
                        .cloned()
                        .collect::<Vec<_>>()
                } else {
                    resources.iter().skip(offset).cloned().collect::<Vec<_>>()
                };

                let next_offset = offset + page_items.len();
                let next_cursor = (next_offset < resources.len()).then(|| next_offset.to_string());

                Ok(ListResourcesPage {
                    items: page_items,
                    next_cursor,
                })
            })
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Metadata, err.to_string()))
    }

    async fn list_fields(&self, resource: &ResourceRef) -> Result<Option<Vec<FieldMeta>>> {
        let schema = quote_identifier(&resource.namespace_id);
        let table = quote_pragma_argument(&resource.resource_id);
        let pragma_sql = format!("PRAGMA {schema}.table_xinfo({table})");

        self.with_connection(|connection| {
            Box::pin(async move {
                let rows = sqlx::query(&pragma_sql).fetch_all(connection).await?;
                if rows.is_empty() {
                    return Ok(None);
                }

                let fields = rows
                    .into_iter()
                    .map(map_field_meta)
                    .collect::<std::result::Result<Vec<_>, sqlx::Error>>()?;

                Ok(Some(fields))
            })
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Metadata, err.to_string()))
    }

    fn text_query_executor(&self) -> Option<&dyn TextQueryExecutor> {
        Some(self)
    }

    fn resource_reader(&self) -> Option<&dyn ResourceReader> {
        Some(self)
    }

    fn text_query_builder(&self) -> Option<&dyn TextQueryBuilder> {
        Some(self)
    }

    fn resource_structure_introspector(&self) -> Option<&dyn ResourceStructureIntrospector> {
        Some(self)
    }
}

#[async_trait]
impl TextQueryExecutor for SqliteConnectionHandle {
    async fn execute_text_query(
        &self,
        _namespace_id: Option<&str>,
        query: &str,
    ) -> Result<QueryResult> {
        let query = query.to_string();
        self.with_connection(|connection| {
            Box::pin(async move {
                let rows = sqlx::query(&query).fetch_all(connection).await?;
                build_query_result(rows)
            })
        })
        .await
        .map_err(|err| ProviderError::new(ProviderErrorKind::Query, err.to_string()))
    }
}

#[async_trait]
impl ResourceReader for SqliteConnectionHandle {
    async fn read_resource(
        &self,
        resource: &ResourceRef,
        request: ReadRequest,
    ) -> Result<QueryResult> {
        let query = build_select_all_query(resource, request.limit, request.offset);
        self.execute_text_query(None, &query).await
    }
}

impl TextQueryBuilder for SqliteConnectionHandle {
    fn build_read_all_query(&self, resource: &ResourceRef) -> Result<Option<String>> {
        Ok(Some(build_select_all_query(resource, None, None)))
    }
}

#[async_trait]
impl ResourceStructureIntrospector for SqliteConnectionHandle {
    async fn list_keys(&self, resource: &ResourceRef) -> Result<Option<Vec<ResourceKeyInfo>>> {
        let indexes = self
            .load_resource_indexes(resource)
            .await
            .map_err(|err| ProviderError::new(ProviderErrorKind::Metadata, err.to_string()))?;

        Ok(Some(
            indexes
                .into_iter()
                .filter_map(|index| match index.origin.as_str() {
                    "pk" => Some(ResourceKeyInfo {
                        name: normalize_key_name(index.name.as_deref()),
                        kind: ResourceKeyKind::Primary,
                        columns: index.columns,
                    }),
                    "u" => Some(ResourceKeyInfo {
                        name: normalize_key_name(index.name.as_deref()),
                        kind: ResourceKeyKind::Unique,
                        columns: index.columns,
                    }),
                    _ => None,
                })
                .collect(),
        ))
    }

    async fn list_indexes(
        &self,
        resource: &ResourceRef,
    ) -> Result<Option<Vec<ResourceIndexInfo>>> {
        let indexes = self
            .load_resource_indexes(resource)
            .await
            .map_err(|err| ProviderError::new(ProviderErrorKind::Metadata, err.to_string()))?;

        Ok(Some(
            indexes
                .into_iter()
                .map(|index| ResourceIndexInfo {
                    name: index.name.unwrap_or_else(|| "<unnamed>".to_string()),
                    columns: index.columns,
                    unique: index.unique,
                })
                .collect(),
        ))
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

    pub fn config(&self) -> &SqliteConfig {
        &self.config
    }

    async fn load_resource_indexes(
        &self,
        resource: &ResourceRef,
    ) -> std::result::Result<Vec<SqliteIndexMeta>, sqlx::Error> {
        let schema = quote_identifier(&resource.namespace_id);
        let table = quote_pragma_argument(&resource.resource_id);
        let index_list_sql = format!("PRAGMA {schema}.index_list({table})");

        self.with_connection(|connection| {
            Box::pin(async move {
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
                    let index_rows = sqlx::query(&index_info_sql).fetch_all(&mut *connection).await?;
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
            })
        })
        .await
    }
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

fn normalize_key_name(name: Option<&str>) -> Option<String> {
    match name {
        Some(name) if name.starts_with("sqlite_autoindex_") => None,
        Some(name) => Some(name.to_string()),
        None => None,
    }
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

fn parse_cursor_offset(cursor: Option<&str>) -> Result<usize> {
    match cursor {
        Some(raw) => raw.parse::<usize>().map_err(|err| {
            ProviderError::invalid_config(format!("invalid resource cursor: {err}"))
        }),
        None => Ok(0),
    }
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
    let ordinal: i64 = row.try_get("cid")?;
    let name: String = row.try_get("name")?;
    let native_type = row.try_get::<Option<String>, _>("type")?;
    let not_null: i64 = row.try_get("notnull")?;
    let default_value_sql = row.try_get::<Option<String>, _>("dflt_value")?;
    let primary_key_ordinal = row.try_get::<i64, _>("pk")?;
    let hidden = row.try_get::<i64, _>("hidden")?;

    Ok(FieldMeta {
        ordinal: usize::try_from(ordinal).ok(),
        name,
        logical_type: native_type
            .as_deref()
            .map(map_logical_type)
            .or(Some(LogicalType::Unknown)),
        native_type,
        nullable: Some(not_null == 0),
        default_value_sql,
        primary_key_ordinal: u32::try_from(primary_key_ordinal)
            .ok()
            .filter(|value| *value > 0),
        hidden: Some(hidden != 0),
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

fn map_logical_type(native_type: &str) -> LogicalType {
    let normalized = native_type.trim().to_ascii_uppercase();

    if normalized.contains("INT") {
        return LogicalType::BigInt;
    }
    if normalized.contains("CHAR") || normalized.contains("CLOB") || normalized.contains("TEXT") {
        return LogicalType::Text;
    }
    if normalized.contains("BLOB") {
        return LogicalType::Binary;
    }
    if normalized.contains("REAL") || normalized.contains("FLOA") || normalized.contains("DOUB") {
        return LogicalType::Double;
    }
    if normalized.contains("DECIMAL") || normalized.contains("NUMERIC") {
        return LogicalType::Decimal;
    }
    if normalized.contains("BOOL") {
        return LogicalType::Bool;
    }
    if normalized.contains("JSON") {
        return LogicalType::Json;
    }
    if normalized.contains("TIMESTAMP") || normalized.contains("DATETIME") {
        return LogicalType::DateTime;
    }
    if normalized == "DATE" {
        return LogicalType::Date;
    }
    if normalized == "TIME" {
        return LogicalType::Time;
    }

    LogicalType::Unknown
}

fn build_query_result(rows: Vec<SqliteRow>) -> std::result::Result<QueryResult, sqlx::Error> {
    if rows.is_empty() {
        return Ok(QueryResult {
            columns: None,
            rows: Vec::new(),
        });
    }

    let columns = build_columns(&rows[0]);
    let result_rows = rows
        .iter()
        .map(build_row_values)
        .collect::<std::result::Result<Vec<_>, sqlx::Error>>()?;

    Ok(QueryResult {
        columns: Some(columns),
        rows: result_rows,
    })
}

fn build_columns(row: &SqliteRow) -> Vec<FieldMeta> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let native_type = column.type_info().name().to_string();
            FieldMeta {
                ordinal: Some(index),
                name: column.name().to_string(),
                logical_type: Some(map_logical_type(&native_type)),
                native_type: Some(native_type),
                nullable: None,
                default_value_sql: None,
                primary_key_ordinal: None,
                hidden: None,
            }
        })
        .collect()
}

fn build_row_values(row: &SqliteRow) -> std::result::Result<QueryRow, sqlx::Error> {
    let mut values = Vec::with_capacity(row.len());

    for index in 0..row.len() {
        let raw = row.try_get_raw(index)?;
        let type_name = raw.type_info().name().to_ascii_uppercase();

        let value = if raw.is_null() {
            Value::Null
        } else if type_name.contains("INT") {
            Value::BigInt(row.try_get(index)?)
        } else if type_name.contains("REAL")
            || type_name.contains("FLOA")
            || type_name.contains("DOUB")
        {
            Value::Double(row.try_get(index)?)
        } else if type_name.contains("BLOB") {
            Value::Binary(row.try_get(index)?)
        } else {
            Value::Text(row.try_get(index)?)
        };

        values.push(value);
    }

    Ok(QueryRow::Fields(values))
}

fn build_select_all_query(
    resource: &ResourceRef,
    limit: Option<u32>,
    offset: Option<u64>,
) -> String {
    let mut query = format!(
        "SELECT * FROM {}.{}",
        quote_identifier(&resource.namespace_id),
        quote_identifier(&resource.resource_id)
    );

    match (limit, offset) {
        (Some(limit), Some(offset)) => {
            query.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));
        }
        (Some(limit), None) => {
            query.push_str(&format!(" LIMIT {limit}"));
        }
        (None, Some(offset)) => {
            query.push_str(&format!(" LIMIT -1 OFFSET {offset}"));
        }
        (None, None) => {}
    }

    query
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_pragma_argument(identifier: &str) -> String {
    format!("'{}'", identifier.replace('\'', "''"))
}

fn like_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pattern_index, mut value_index) = (0usize, 0usize);
    let mut star_index = None;
    let mut match_index = 0usize;

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'_' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'%' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            match_index = value_index;
        } else if let Some(star_index) = star_index {
            pattern_index = star_index + 1;
            match_index += 1;
            value_index = match_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'%' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use kandb_provider_core::{
        build_read_all_query, execute_text_query, read_resource, ResourceKeyKind,
    };
    use tempfile::NamedTempFile;

    #[test]
    fn test_connection_supports_memory_config() {
        block_on(async {
            let provider = SqliteProvider;
            provider
                .test_connection(&SqliteConfig::default())
                .await
                .unwrap();
        });
    }

    #[test]
    fn path_config_respects_create_if_missing() {
        block_on(async {
            let provider = SqliteProvider;
            let temp = NamedTempFile::new().unwrap();
            let path = temp.path().with_extension("sqlite");
            drop(temp);

            let error = provider
                .connect(SqliteConfig {
                    location: SqliteLocation::Path(path),
                    read_only: false,
                    create_if_missing: false,
                })
                .await;

            let error = match error {
                Ok(_) => panic!("expected connect failure"),
                Err(error) => error,
            };

            assert_eq!(error.kind(), ProviderErrorKind::Connect);
        });
    }

    #[test]
    fn read_only_connections_do_not_fail_wal_setup() {
        block_on(async {
            let provider = SqliteProvider;
            let temp = NamedTempFile::new().unwrap();
            let path = temp.path().to_path_buf();

            provider
                .connect(SqliteConfig {
                    location: SqliteLocation::Path(path.clone()),
                    read_only: false,
                    create_if_missing: true,
                })
                .await
                .unwrap();

            provider
                .test_connection(&SqliteConfig {
                    location: SqliteLocation::Path(path),
                    read_only: true,
                    create_if_missing: false,
                })
                .await
                .unwrap();
        });
    }

    #[test]
    fn attached_databases_show_up_as_namespaces() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            execute_text_query(&connection, None, "ATTACH DATABASE ':memory:' AS aux")
                .await
                .unwrap();

            let namespaces = connection.list_namespaces().await.unwrap();
            let names = namespaces
                .into_iter()
                .map(|namespace| namespace.name)
                .collect::<Vec<_>>();

            assert!(names.contains(&"main".to_string()));
            assert!(names.contains(&"aux".to_string()));
        });
    }

    #[test]
    fn list_resources_uses_sqlite_metadata_kinds() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            execute_text_query(
                &connection,
                None,
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "CREATE VIEW user_names AS SELECT name FROM users",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "CREATE VIRTUAL TABLE docs USING fts5(title, body)",
            )
            .await
            .unwrap();

            let page = connection
                .list_resources(
                    "main",
                    ListResourcesRequest {
                        cursor: None,
                        limit: None,
                        pattern: None,
                    },
                )
                .await
                .unwrap();

            assert!(
                page.items
                    .iter()
                    .any(|item| item.name == "users" && item.kind == ResourceKind::Table)
            );
            assert!(
                page.items
                    .iter()
                    .any(|item| item.name == "user_names" && item.kind == ResourceKind::View)
            );
            assert!(
                page.items
                    .iter()
                    .any(|item| item.name == "docs" && item.kind == ResourceKind::VirtualTable)
            );
            assert!(
                page
                    .items
                    .iter()
                    .any(|item| item.name == "sqlite_schema" && item.kind == ResourceKind::Table)
            );
        });
    }

    #[test]
    fn list_keys_and_indexes_include_unique_constraints_and_autoindexes() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            execute_text_query(
                &connection,
                None,
                "CREATE TABLE novel_tag (
                    novel_id INTEGER NOT NULL,
                    tag_id INTEGER NOT NULL,
                    PRIMARY KEY (novel_id, tag_id)
                )",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "CREATE TABLE tag (
                    id INTEGER PRIMARY KEY,
                    name TEXT UNIQUE
                )",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "CREATE INDEX tag_name_manual_idx ON tag(name)",
            )
            .await
            .unwrap();

            let tag_resource = ResourceRef {
                namespace_id: "main".into(),
                resource_id: "tag".into(),
            };
            let tag_keys = connection.list_keys(&tag_resource).await.unwrap().unwrap();
            let tag_indexes = connection.list_indexes(&tag_resource).await.unwrap().unwrap();

            assert!(
                tag_keys.iter().any(|key| key.name.is_none()
                    && key.kind == ResourceKeyKind::Unique
                    && key.columns == vec!["name".to_string()])
            );
            assert!(
                tag_indexes.iter().any(|index| index.name == "sqlite_autoindex_tag_1"
                    && index.columns == vec!["name".to_string()]
                    && index.unique)
            );
            assert!(
                tag_indexes.iter().any(|index| index.name == "tag_name_manual_idx"
                    && index.columns == vec!["name".to_string()]
                    && !index.unique)
            );

            let novel_tag_keys = connection
                .list_keys(&ResourceRef {
                    namespace_id: "main".into(),
                    resource_id: "novel_tag".into(),
                })
                .await
                .unwrap()
                .unwrap();

            assert!(
                novel_tag_keys.iter().any(|key| key.kind == ResourceKeyKind::Primary
                    && key.columns == vec!["novel_id".to_string(), "tag_id".to_string()])
            );
        });
    }

    #[test]
    fn list_fields_reports_generated_and_hidden_columns() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            execute_text_query(
                &connection,
                None,
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, slug TEXT GENERATED ALWAYS AS (lower(name)) VIRTUAL)",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "CREATE VIRTUAL TABLE docs USING fts5(title, body)",
            )
            .await
            .unwrap();

            let users_fields = connection
                .list_fields(&ResourceRef {
                    namespace_id: "main".into(),
                    resource_id: "users".into(),
                })
                .await
                .unwrap()
                .unwrap();
            assert!(
                users_fields
                    .iter()
                    .any(|field| field.name == "slug" && field.hidden == Some(true))
            );

            let docs_fields = connection
                .list_fields(&ResourceRef {
                    namespace_id: "main".into(),
                    resource_id: "docs".into(),
                })
                .await
                .unwrap()
                .unwrap();
            assert!(docs_fields.iter().any(|field| field.hidden == Some(true)));
        });
    }

    #[test]
    fn query_results_preserve_duplicate_column_names() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            execute_text_query(
                &connection,
                None,
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "INSERT INTO users (name) VALUES ('alice')",
            )
            .await
            .unwrap();

            let result = execute_text_query(
                &connection,
                None,
                "SELECT u1.id, u2.id FROM users u1 JOIN users u2 ON u1.id = u2.id",
            )
            .await
            .unwrap();

            let columns = result.columns.unwrap();
            assert_eq!(columns[0].name, "id");
            assert_eq!(columns[1].name, "id");
        });
    }

    #[test]
    fn value_mapping_covers_sqlite_storage_classes() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            let result = execute_text_query(
                &connection,
                None,
                "SELECT CAST(1 AS INTEGER) AS int_col, CAST(1.5 AS REAL) AS real_col, 'txt' AS text_col, X'CAFE' AS blob_col, NULL AS null_col",
            )
            .await
            .unwrap();

            match &result.rows[0] {
                QueryRow::Fields(values) => {
                    assert_eq!(values[0], Value::BigInt(1));
                    assert_eq!(values[1], Value::Double(1.5));
                    assert_eq!(values[2], Value::Text("txt".into()));
                    assert_eq!(values[3], Value::Binary(vec![0xCA, 0xFE]));
                    assert_eq!(values[4], Value::Null);
                }
                row => panic!("expected fields row, got {row:?}"),
            }
        });
    }

    #[test]
    fn read_resource_and_builder_apply_limit_and_offset() {
        block_on(async {
            let provider = SqliteProvider;
            let connection = provider.connect(SqliteConfig::default()).await.unwrap();

            execute_text_query(
                &connection,
                None,
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            )
            .await
            .unwrap();
            execute_text_query(
                &connection,
                None,
                "INSERT INTO users (name) VALUES ('alice'), ('bob'), ('carol')",
            )
            .await
            .unwrap();

            let query = build_read_all_query(
                &connection,
                &ResourceRef {
                    namespace_id: "main".into(),
                    resource_id: "users".into(),
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(query, "SELECT * FROM \"main\".\"users\"");

            let result = read_resource(
                &connection,
                &ResourceRef {
                    namespace_id: "main".into(),
                    resource_id: "users".into(),
                },
                ReadRequest {
                    limit: Some(1),
                    offset: Some(1),
                },
            )
            .await
            .unwrap();

            assert_eq!(result.rows.len(), 1);
            match &result.rows[0] {
                QueryRow::Fields(values) => {
                    assert_eq!(values[1], Value::Text("bob".into()));
                }
                row => panic!("expected fields row, got {row:?}"),
            }
        });
    }

    #[test]
    fn build_select_all_query_adds_limit_minus_one_for_offset_only() {
        let query = build_select_all_query(
            &ResourceRef {
                namespace_id: "main".into(),
                resource_id: "users".into(),
            },
            None,
            Some(10),
        );

        assert_eq!(query, "SELECT * FROM \"main\".\"users\" LIMIT -1 OFFSET 10");
    }
}
