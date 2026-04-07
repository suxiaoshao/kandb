use crate::{
    config::CONFIG_FILE_NAME,
    errors::{KandbError, KandbResult},
};
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

pub(crate) const APP_DIR_NAME: &str = "kandb";
pub(crate) const STATE_DIR_NAME: &str = "state";
pub(crate) const WORKSPACE_STATE_FILE_NAME: &str = "workspace_state.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppPaths {
    config_dir: PathBuf,
    config_file: PathBuf,
    data_dir: PathBuf,
    state_dir: PathBuf,
    workspace_state_file: PathBuf,
}

impl AppPaths {
    pub(crate) fn discover() -> KandbResult<Self> {
        let config_root = dirs_next::config_dir().ok_or(KandbError::ConfigDirNotAvailable)?;
        let data_root = dirs_next::data_local_dir().ok_or(KandbError::DataDirNotAvailable)?;

        Ok(Self::from_roots(
            config_root.join(APP_DIR_NAME),
            data_root.join(APP_DIR_NAME),
        ))
    }

    pub(crate) fn from_roots(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        let config_file = config_dir.join(CONFIG_FILE_NAME);
        let state_dir = data_dir.join(STATE_DIR_NAME);
        let workspace_state_file = state_dir.join(WORKSPACE_STATE_FILE_NAME);
        Self {
            config_dir,
            config_file,
            data_dir,
            state_dir,
            workspace_state_file,
        }
    }

    pub(crate) fn ensure_dirs(&self) -> KandbResult<()> {
        ensure_dir(&self.config_dir)?;
        ensure_dir(&self.data_dir)?;
        ensure_dir(&self.state_dir)?;
        Ok(())
    }

    pub(crate) fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub(crate) fn config_file(&self) -> &Path {
        &self.config_file
    }

    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub(crate) fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub(crate) fn workspace_state_file(&self) -> &Path {
        &self.workspace_state_file
    }
}

fn ensure_dir(path: &Path) -> KandbResult<()> {
    if path.exists() {
        return Ok(());
    }

    create_dir_all(path).map_err(|source| KandbError::CreateDir {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{APP_DIR_NAME, AppPaths, STATE_DIR_NAME, WORKSPACE_STATE_FILE_NAME};
    use crate::config::CONFIG_FILE_NAME;
    use std::path::PathBuf;

    #[test]
    fn app_paths_use_separate_config_and_data_roots() {
        let paths = AppPaths::from_roots(
            PathBuf::from("/tmp/config-root").join(APP_DIR_NAME),
            PathBuf::from("/tmp/data-root").join(APP_DIR_NAME),
        );

        assert_eq!(paths.config_dir(), PathBuf::from("/tmp/config-root/kandb"));
        assert_eq!(
            paths.config_file(),
            PathBuf::from("/tmp/config-root/kandb").join(CONFIG_FILE_NAME)
        );
        assert_eq!(paths.data_dir(), PathBuf::from("/tmp/data-root/kandb"));
        assert_eq!(
            paths.state_dir(),
            PathBuf::from("/tmp/data-root/kandb").join(STATE_DIR_NAME)
        );
        assert_eq!(
            paths.workspace_state_file(),
            PathBuf::from("/tmp/data-root/kandb")
                .join(STATE_DIR_NAME)
                .join(WORKSPACE_STATE_FILE_NAME)
        );
    }
}
