mod error;
mod model;
mod registry;
mod traits;

pub use error::{ProviderError, ProviderErrorKind, Result};
pub use model::{
    FieldMeta, ListResourcesPage, ListResourcesRequest, LogicalType, NativeValue, NamespaceInfo,
    NamespaceKind, QueryResult, QueryRow, ReadRequest, ResourceInfo, ResourceKind, ResourceRef,
    Value,
};
pub use registry::ProviderRegistry;
pub use traits::{
    Connection, ErasedProviderFactory, ProviderFactory, ResourceReader, TextQueryBuilder,
    TextQueryExecutor, build_read_all_query, execute_text_query, read_resource,
};

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::executor::block_on;
    use indexmap::IndexMap;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct MockConfig {
        dsn: String,
    }

    struct MockFactory;

    #[async_trait]
    impl ProviderFactory for MockFactory {
        type Config = MockConfig;
        type Connection = MockConnection;

        fn kind(&self) -> &'static str {
            "mock"
        }

        fn display_name(&self) -> &'static str {
            "Mock"
        }

        async fn connect(&self, config: Self::Config) -> Result<Self::Connection> {
            if config.dsn == "bad" {
                return Err(ProviderError::new(
                    ProviderErrorKind::Connect,
                    "mock connect failed",
                ));
            }

            Ok(MockConnection)
        }

        async fn test_connection(&self, config: &Self::Config) -> Result<()> {
            if config.dsn == "bad" {
                return Err(ProviderError::new(
                    ProviderErrorKind::Authenticate,
                    "mock auth failed",
                ));
            }

            Ok(())
        }
    }

    struct MockConnection;

    #[async_trait]
    impl Connection for MockConnection {
        fn kind(&self) -> &'static str {
            "mock"
        }

        async fn ping(&self) -> Result<()> {
            Ok(())
        }

        async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>> {
            Ok(vec![NamespaceInfo {
                id: "public".into(),
                name: "public".into(),
                kind: NamespaceKind::Schema,
                parent_id: None,
            }])
        }

        async fn list_resources(
            &self,
            namespace_id: &str,
            request: ListResourcesRequest,
        ) -> Result<ListResourcesPage> {
            let first_page = request.cursor.is_none();
            let table_name = if first_page { "users" } else { "orders" };

            Ok(ListResourcesPage {
                items: vec![ResourceInfo {
                    resource: ResourceRef {
                        namespace_id: namespace_id.into(),
                        resource_id: table_name.into(),
                    },
                    name: table_name.into(),
                    kind: ResourceKind::Table,
                }],
                next_cursor: first_page.then(|| "cursor-2".into()),
            })
        }

        async fn list_fields(&self, resource: &ResourceRef) -> Result<Option<Vec<FieldMeta>>> {
            if resource.resource_id == "documents" {
                return Ok(None);
            }

            Ok(Some(vec![
                FieldMeta {
                    name: "id".into(),
                    logical_type: Some(LogicalType::Int),
                    native_type: Some("INTEGER".into()),
                    nullable: Some(false),
                },
                FieldMeta {
                    name: "id".into(),
                    logical_type: Some(LogicalType::Int),
                    native_type: Some("INTEGER".into()),
                    nullable: Some(false),
                },
            ]))
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
    }

    #[async_trait]
    impl TextQueryExecutor for MockConnection {
        async fn execute_text_query(
            &self,
            _namespace_id: Option<&str>,
            _query: &str,
        ) -> Result<QueryResult> {
            Ok(QueryResult {
                columns: Some(vec![
                    FieldMeta {
                        name: "id".into(),
                        logical_type: Some(LogicalType::Int),
                        native_type: Some("INTEGER".into()),
                        nullable: Some(false),
                    },
                    FieldMeta {
                        name: "id".into(),
                        logical_type: Some(LogicalType::Int),
                        native_type: Some("INTEGER".into()),
                        nullable: Some(false),
                    },
                ]),
                rows: vec![QueryRow::Fields(vec![Value::Integer(1), Value::Integer(2)])],
            })
        }
    }

    #[async_trait]
    impl ResourceReader for MockConnection {
        async fn read_resource(
            &self,
            resource: &ResourceRef,
            _request: ReadRequest,
        ) -> Result<QueryResult> {
            let rows = match resource.resource_id.as_str() {
                "redis:session:1" => {
                    let mut document = IndexMap::new();
                    document.insert("key".into(), Value::Text("session:1".into()));
                    document.insert("type".into(), Value::Text("hash".into()));
                    document.insert("ttl".into(), Value::Integer(60));
                    document.insert(
                        "value".into(),
                        Value::Object(IndexMap::from([
                            ("user_id".into(), Value::Integer(7)),
                            ("active".into(), Value::Bool(true)),
                        ])),
                    );
                    vec![QueryRow::Document(document)]
                }
                "documents" => {
                    let mut document = IndexMap::new();
                    document.insert("_id".into(), Value::Text("abc".into()));
                    document.insert("name".into(), Value::Text("mongo".into()));
                    vec![QueryRow::Document(document)]
                }
                _ => vec![QueryRow::Fields(vec![Value::Integer(1), Value::Text("alice".into())])],
            };

            Ok(QueryResult {
                columns: None,
                rows,
            })
        }
    }

    impl TextQueryBuilder for MockConnection {
        fn build_read_all_query(&self, resource: &ResourceRef) -> Result<Option<String>> {
            Ok(Some(format!(
                "SELECT * FROM {}.{}",
                resource.namespace_id, resource.resource_id
            )))
        }
    }

    struct ReadOnlyConnection;

    #[async_trait]
    impl Connection for ReadOnlyConnection {
        fn kind(&self) -> &'static str {
            "readonly"
        }

        async fn ping(&self) -> Result<()> {
            Ok(())
        }

        async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>> {
            Ok(Vec::new())
        }

        async fn list_resources(
            &self,
            _namespace_id: &str,
            _request: ListResourcesRequest,
        ) -> Result<ListResourcesPage> {
            Ok(ListResourcesPage::default())
        }

        async fn list_fields(&self, _resource: &ResourceRef) -> Result<Option<Vec<FieldMeta>>> {
            Ok(None)
        }
    }

    #[test]
    fn erased_factory_roundtrip_and_registry_connect() {
        block_on(async {
            let mut registry = ProviderRegistry::new();
            registry.register(Arc::new(MockFactory)).unwrap();

            let connection = registry
                .connect(
                    "mock",
                    serde_json::json!({
                        "dsn": "memory://demo"
                    }),
                )
                .await
                .unwrap();

            assert_eq!(registry.kinds(), vec!["mock"]);
            assert_eq!(connection.kind(), "mock");
            connection.ping().await.unwrap();
        });
    }

    #[test]
    fn invalid_config_maps_to_invalid_config_error() {
        block_on(async {
            let factory = MockFactory;
            let error = match factory
                .connect_erased(serde_json::json!({ "missing": "dsn" }))
                .await
            {
                Ok(_) => panic!("expected invalid config error"),
                Err(error) => error,
            };

            assert_eq!(error.kind(), ProviderErrorKind::InvalidConfig);
        });
    }

    #[test]
    fn sql_results_keep_duplicate_columns_and_row_order() {
        block_on(async {
            let connection = MockConnection;
            let result = execute_text_query(&connection, Some("public"), "SELECT 1").await.unwrap();

            let columns = result.columns.unwrap();
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0].name, "id");
            assert_eq!(columns[1].name, "id");

            match &result.rows[0] {
                QueryRow::Fields(values) => {
                    assert_eq!(values, &vec![Value::Integer(1), Value::Integer(2)]);
                }
                row => panic!("expected fields row, got {row:?}"),
            }
        });
    }

    #[test]
    fn mongo_style_resources_can_omit_fields() {
        block_on(async {
            let connection = MockConnection;
            let resource = ResourceRef {
                namespace_id: "app".into(),
                resource_id: "documents".into(),
            };

            assert_eq!(connection.list_fields(&resource).await.unwrap(), None);

            let result = read_resource(&connection, &resource, ReadRequest::default())
                .await
                .unwrap();

            match &result.rows[0] {
                QueryRow::Document(document) => {
                    assert_eq!(document.get("_id"), Some(&Value::Text("abc".into())));
                }
                row => panic!("expected document row, got {row:?}"),
            }
        });
    }

    #[test]
    fn redis_style_resources_can_page_and_return_documents() {
        block_on(async {
            let connection = MockConnection;
            let first_page = connection
                .list_resources(
                    "0",
                    ListResourcesRequest {
                        cursor: None,
                        limit: Some(1),
                        pattern: Some("session:*".into()),
                    },
                )
                .await
                .unwrap();

            assert_eq!(first_page.items.len(), 1);
            assert_eq!(first_page.next_cursor.as_deref(), Some("cursor-2"));

            let second_page = connection
                .list_resources(
                    "0",
                    ListResourcesRequest {
                        cursor: first_page.next_cursor,
                        limit: Some(1),
                        pattern: Some("session:*".into()),
                    },
                )
                .await
                .unwrap();

            assert_eq!(second_page.items[0].name, "orders");

            let redis_result = read_resource(
                &connection,
                &ResourceRef {
                    namespace_id: "0".into(),
                    resource_id: "redis:session:1".into(),
                },
                ReadRequest::default(),
            )
            .await
            .unwrap();

            match &redis_result.rows[0] {
                QueryRow::Document(document) => {
                    assert_eq!(document.get("type"), Some(&Value::Text("hash".into())));
                }
                row => panic!("expected document row, got {row:?}"),
            }
        });
    }

    #[test]
    fn unsupported_capabilities_return_expected_error() {
        block_on(async {
            let connection = ReadOnlyConnection;
            let error = execute_text_query(&connection, None, "PING")
                .await
                .unwrap_err();

            assert_eq!(error.kind(), ProviderErrorKind::UnsupportedCapability);
        });
    }

    #[test]
    fn value_datetime_variants_roundtrip_via_serde() {
        let value = Value::Object(IndexMap::from([
            (
                "date".into(),
                Value::Date(Date::from_calendar_date(2026, Month::April, 6).unwrap()),
            ),
            ("time".into(), Value::Time(Time::from_hms(1, 2, 3).unwrap())),
            (
                "datetime".into(),
                Value::DateTime(PrimitiveDateTime::new(
                    Date::from_calendar_date(2026, Month::April, 6).unwrap(),
                    Time::from_hms(1, 2, 3).unwrap(),
                )),
            ),
            (
                "datetime_tz".into(),
                Value::DateTimeTz(
                    OffsetDateTime::from_unix_timestamp(1_775_440_923).unwrap(),
                ),
            ),
            ("decimal".into(), Value::Decimal("12.34".into())),
            (
                "native".into(),
                Value::Native(NativeValue {
                    type_name: "jsonb".into(),
                    repr: serde_json::json!({"k": "v"}),
                }),
            ),
        ]));

        let json = serde_json::to_string(&value).unwrap();
        let decoded: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn read_all_query_builder_is_available_for_sql_style_connections() {
        let connection = MockConnection;
        let resource = ResourceRef {
            namespace_id: "public".into(),
            resource_id: "users".into(),
        };

        let query = build_read_all_query(&connection, &resource).unwrap();
        assert_eq!(query.as_deref(), Some("SELECT * FROM public.users"));
    }
}
