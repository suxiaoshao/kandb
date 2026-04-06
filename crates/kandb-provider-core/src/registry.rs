use crate::{Connection, ErasedProviderFactory, ProviderError, ProviderErrorKind, Result};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct ProviderRegistry {
    factories: HashMap<String, Arc<dyn ErasedProviderFactory>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, factory: Arc<dyn ErasedProviderFactory>) -> Result<()> {
        let kind = factory.kind();

        if self.factories.contains_key(kind) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidConfig,
                format!("provider `{kind}` is already registered"),
            ));
        }

        self.factories.insert(kind.to_string(), factory);
        Ok(())
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn ErasedProviderFactory>> {
        self.factories.get(kind).cloned()
    }

    pub fn kinds(&self) -> Vec<&str> {
        let mut kinds = self
            .factories
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
        let factory = self.get(kind).ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidConfig,
                format!("provider `{kind}` is not registered"),
            )
        })?;

        factory.connect_erased(config).await
    }
}
