use crate::{Connection, ErasedProviderPlugin, ProviderError, ProviderErrorKind, Result};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct ProviderRegistry {
    plugins: HashMap<String, Arc<dyn ErasedProviderPlugin>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plugin: Arc<dyn ErasedProviderPlugin>) -> Result<()> {
        let kind = plugin.kind();

        if self.plugins.contains_key(kind) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidConfig,
                format!("provider `{kind}` is already registered"),
            ));
        }

        self.plugins.insert(kind.to_string(), plugin);
        Ok(())
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn ErasedProviderPlugin>> {
        self.plugins.get(kind).cloned()
    }

    pub fn kinds(&self) -> Vec<&str> {
        let mut kinds = self
            .plugins
            .keys()
            .map(std::string::String::as_str)
            .collect::<Vec<_>>();
        kinds.sort_unstable();
        kinds
    }

    pub async fn connect(
        &self,
        kind: &str,
        config: serde_json::Value,
    ) -> Result<Box<dyn Connection>> {
        let plugin = self.get(kind).ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidConfig,
                format!("provider `{kind}` is not registered"),
            )
        })?;

        plugin.connect_erased(config).await
    }
}
