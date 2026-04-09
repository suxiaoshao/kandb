mod error;
mod model;
mod registry;
mod traits;

pub use error::{ProviderError, ProviderErrorKind, Result};
pub use model::{
    ConnectionFormSchema, FormField, FormFieldKind, FormSelectOption, IconToken, SidebarTree,
    TreeChildren, TreeNode,
};
pub use registry::ProviderRegistry;
pub use traits::{Connection, ErasedProviderPlugin, ProviderPlugin};

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::executor::block_on;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct MockConfig {
        dsn: String,
    }

    struct MockPlugin;

    #[async_trait]
    impl ProviderPlugin for MockPlugin {
        type Config = MockConfig;
        type Connection = MockConnection;

        fn kind(&self) -> &'static str {
            "mock"
        }

        fn display_name(&self) -> &'static str {
            "Mock"
        }

        fn connection_form(&self, locale: &str) -> ConnectionFormSchema {
            ConnectionFormSchema {
                title: format!("Mock {locale}"),
                fields: vec![FormField {
                    key: "dsn".into(),
                    label: "DSN".into(),
                    kind: FormFieldKind::Text,
                    required: true,
                    help_text: None,
                    placeholder: Some("memory://demo".into()),
                    options: Vec::new(),
                }],
            }
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

        async fn load_sidebar(&self, _locale: &str) -> Result<SidebarTree> {
            Ok(SidebarTree {
                roots: vec![TreeNode {
                    id: "root".into(),
                    label: "Root".into(),
                    icon: IconToken::Database,
                    children: TreeChildren::Branch(vec![TreeNode {
                        id: "leaf".into(),
                        label: "Leaf".into(),
                        icon: IconToken::Table,
                        children: TreeChildren::Leaf,
                    }]),
                }],
            })
        }
    }

    #[test]
    fn erased_plugin_roundtrip_and_registry_connect() {
        block_on(async {
            let mut registry = ProviderRegistry::new();
            registry.register(Arc::new(MockPlugin)).unwrap();

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
            let plugin = MockPlugin;
            let error = match plugin
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
    fn connection_form_is_available_through_erased_plugin() {
        let plugin: Arc<dyn ErasedProviderPlugin> = Arc::new(MockPlugin);
        let form = plugin.connection_form("en-US");

        assert_eq!(form.title, "Mock en-US");
        assert_eq!(form.fields[0].key, "dsn");
    }

    #[test]
    fn sidebar_tree_roundtrips_via_serde() {
        let tree = SidebarTree {
            roots: vec![TreeNode {
                id: "root".into(),
                label: "Root".into(),
                icon: IconToken::Folder,
                children: TreeChildren::Branch(vec![TreeNode {
                    id: "leaf".into(),
                    label: "Leaf".into(),
                    icon: IconToken::Column,
                    children: TreeChildren::Leaf,
                }]),
            }],
        };

        let json = serde_json::to_string(&tree).unwrap();
        let decoded: SidebarTree = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, tree);
    }

    #[test]
    fn tree_children_leaf_and_empty_branch_stay_distinct() {
        let leaf = TreeChildren::Leaf;
        let branch = TreeChildren::Branch(Vec::new());

        assert_ne!(leaf, branch);

        let leaf_json = serde_json::to_string(&leaf).unwrap();
        let branch_json = serde_json::to_string(&branch).unwrap();

        assert_eq!(
            serde_json::from_str::<TreeChildren>(&leaf_json).unwrap(),
            leaf
        );
        assert_eq!(
            serde_json::from_str::<TreeChildren>(&branch_json).unwrap(),
            branch
        );
    }
}
