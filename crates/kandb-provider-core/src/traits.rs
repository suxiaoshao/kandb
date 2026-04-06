use crate::{
    FieldMeta, ListResourcesPage, ListResourcesRequest, ProviderError, ProviderErrorKind,
    QueryResult, ReadRequest, ResourceRef, Result,
};
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

#[async_trait]
pub trait ProviderFactory: Send + Sync + 'static {
    type Config: Serialize + DeserializeOwned + Send + Sync + 'static;
    type Connection: Connection + 'static;

    fn kind(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn connect(&self, config: Self::Config) -> Result<Self::Connection>;
    async fn test_connection(&self, config: &Self::Config) -> Result<()>;
}

#[async_trait]
pub trait ErasedProviderFactory: Send + Sync {
    fn kind(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn connect_erased(&self, config: serde_json::Value) -> Result<Box<dyn Connection>>;
    async fn test_connection_erased(&self, config: serde_json::Value) -> Result<()>;
}

#[async_trait]
impl<T> ErasedProviderFactory for T
where
    T: ProviderFactory,
{
    fn kind(&self) -> &'static str {
        ProviderFactory::kind(self)
    }

    fn display_name(&self) -> &'static str {
        ProviderFactory::display_name(self)
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
    async fn list_namespaces(&self) -> Result<Vec<crate::NamespaceInfo>>;
    async fn list_resources(
        &self,
        namespace_id: &str,
        request: ListResourcesRequest,
    ) -> Result<ListResourcesPage>;
    async fn list_fields(&self, resource: &ResourceRef) -> Result<Option<Vec<FieldMeta>>>;

    fn text_query_executor(&self) -> Option<&dyn TextQueryExecutor> {
        None
    }

    fn resource_reader(&self) -> Option<&dyn ResourceReader> {
        None
    }

    fn text_query_builder(&self) -> Option<&dyn TextQueryBuilder> {
        None
    }
}

#[async_trait]
pub trait TextQueryExecutor: Send + Sync {
    async fn execute_text_query(
        &self,
        namespace_id: Option<&str>,
        query: &str,
    ) -> Result<QueryResult>;
}

#[async_trait]
pub trait ResourceReader: Send + Sync {
    async fn read_resource(
        &self,
        resource: &ResourceRef,
        request: ReadRequest,
    ) -> Result<QueryResult>;
}

pub trait TextQueryBuilder: Send + Sync {
    fn build_read_all_query(&self, resource: &ResourceRef) -> Result<Option<String>>;
}

pub async fn execute_text_query(
    connection: &dyn Connection,
    namespace_id: Option<&str>,
    query: &str,
) -> Result<QueryResult> {
    let executor = connection.text_query_executor().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::UnsupportedCapability,
            format!(
                "provider `{}` does not support text queries",
                connection.kind()
            ),
        )
    })?;

    executor.execute_text_query(namespace_id, query).await
}

pub async fn read_resource(
    connection: &dyn Connection,
    resource: &ResourceRef,
    request: ReadRequest,
) -> Result<QueryResult> {
    let reader = connection.resource_reader().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::UnsupportedCapability,
            format!(
                "provider `{}` does not support reading resources",
                connection.kind()
            ),
        )
    })?;

    reader.read_resource(resource, request).await
}

pub fn build_read_all_query(
    connection: &dyn Connection,
    resource: &ResourceRef,
) -> Result<Option<String>> {
    let builder = connection.text_query_builder().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::UnsupportedCapability,
            format!(
                "provider `{}` does not support building text queries",
                connection.kind()
            ),
        )
    })?;

    builder.build_read_all_query(resource)
}
