use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProviderError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorKind {
    InvalidConfig,
    Connect,
    Authenticate,
    Ping,
    Metadata,
    Query,
    UnsupportedCapability,
    UnsupportedValue,
    Timeout,
}

#[derive(Debug, Error)]
#[error("{kind:?}: {message}")]
pub struct ProviderError {
    kind: ProviderErrorKind,
    message: String,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::new(ProviderErrorKind::InvalidConfig, message)
    }

    pub fn unsupported_capability(message: impl Into<String>) -> Self {
        Self::new(ProviderErrorKind::UnsupportedCapability, message)
    }

    pub fn kind(&self) -> ProviderErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
