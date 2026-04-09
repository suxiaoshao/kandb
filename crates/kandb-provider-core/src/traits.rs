use crate::{ConnectionFormSchema, ProviderError, Result, SidebarTree};
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

#[async_trait]
pub trait ProviderPlugin: Send + Sync + 'static {
    type Config: Serialize + DeserializeOwned + Send + Sync + 'static;
    type Connection: Connection + 'static;

    fn kind(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn connection_form(&self, locale: &str) -> ConnectionFormSchema;

    async fn connect(&self, config: Self::Config) -> Result<Self::Connection>;
    async fn test_connection(&self, config: &Self::Config) -> Result<()>;
}

#[async_trait]
pub trait ErasedProviderPlugin: Send + Sync {
    fn kind(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn connection_form(&self, locale: &str) -> ConnectionFormSchema;

    async fn connect_erased(&self, config: serde_json::Value) -> Result<Box<dyn Connection>>;
    async fn test_connection_erased(&self, config: serde_json::Value) -> Result<()>;
}

#[async_trait]
impl<T> ErasedProviderPlugin for T
where
    T: ProviderPlugin,
{
    fn kind(&self) -> &'static str {
        ProviderPlugin::kind(self)
    }

    fn display_name(&self) -> &'static str {
        ProviderPlugin::display_name(self)
    }

    fn connection_form(&self, locale: &str) -> ConnectionFormSchema {
        ProviderPlugin::connection_form(self, locale)
    }

    async fn connect_erased(&self, config: serde_json::Value) -> Result<Box<dyn Connection>> {
        let config = serde_json::from_value(config)
            .map_err(|err| ProviderError::invalid_config(err.to_string()))?;
        let connection = self.connect(config).await?;
        Ok(Box::new(connection))
    }

    async fn test_connection_erased(&self, config: serde_json::Value) -> Result<()> {
        let config = serde_json::from_value(config)
            .map_err(|err| ProviderError::invalid_config(err.to_string()))?;
        self.test_connection(&config).await
    }
}

#[async_trait]
pub trait Connection: Send + Sync {
    fn kind(&self) -> &'static str;

    async fn ping(&self) -> Result<()>;
    async fn load_sidebar(&self, locale: &str) -> Result<SidebarTree>;
}
