use thiserror::Error;

pub(crate) type KandbResult<T> = Result<T, KandbError>;

#[derive(Debug, Error)]
pub(crate) enum KandbError {
    #[error("log file path is not available")]
    LogFileNotFound,
}
