use std::{io, path::PathBuf};
use thiserror::Error;

pub(crate) type KandbResult<T> = Result<T, KandbError>;

#[derive(Debug, Error)]
pub(crate) enum KandbError {
    #[error("log file path is not available")]
    LogFileNotFound,
    #[error("config directory is not available")]
    ConfigDirNotAvailable,
    #[error("data directory is not available")]
    DataDirNotAvailable,
    #[error("home directory is not available")]
    HomeDirNotAvailable,
    #[error("failed to create directory `{path}`")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read config file `{path}`")]
    ReadConfigFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write config file `{path}`")]
    WriteConfigFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse config file `{path}`: {message}")]
    ParseConfigFile { path: PathBuf, message: String },
    #[error("failed to serialize config file `{path}`: {message}")]
    SerializeConfig { path: PathBuf, message: String },
    #[error("failed to read workspace state file `{path}`")]
    ReadWorkspaceStateFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write workspace state file `{path}`")]
    WriteWorkspaceStateFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse workspace state file `{path}`: {message}")]
    ParseWorkspaceStateFile { path: PathBuf, message: String },
    #[error("failed to serialize workspace state file `{path}`: {message}")]
    SerializeWorkspaceStateFile { path: PathBuf, message: String },
    #[error("unsupported config version `{version}`")]
    UnsupportedConfigVersion { version: u32 },
    #[error("duplicate connection id `{0}` in config")]
    DuplicateConnectionId(String),
    #[error("default connection `{0}` does not exist")]
    MissingDefaultConnection(String),
    #[error(
        "failed to decode provider config for connection `{connection_id}` (`{provider}`): {message}"
    )]
    ProviderConfigDecode {
        connection_id: String,
        provider: String,
        message: String,
    },
}
