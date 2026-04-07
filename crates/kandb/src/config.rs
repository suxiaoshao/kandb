use crate::{
    app_paths::AppPaths,
    errors::{KandbError, KandbResult},
};
use gpui::Global;
use kandb_provider_sqlite::{SqliteConfig, SqliteLocation};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub(crate) const CONFIG_FILE_NAME: &str = "config.toml";
const CONFIG_VERSION: u32 = 1;
const SQLITE_PROVIDER: &str = "sqlite";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AppConfigFile {
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) default_connection_id: Option<String>,
    #[serde(default)]
    pub(crate) connections: Vec<StoredConnectionProfile>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoadedAppConfig {
    pub(crate) paths: AppPaths,
    pub(crate) file: AppConfigFile,
    pub(crate) resolved_connections: Vec<ResolvedConnectionProfile>,
}

impl Global for LoadedAppConfig {}

impl Default for AppConfigFile {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            default_connection_id: None,
            connections: Vec::new(),
        }
    }
}

impl AppConfigFile {
    pub(crate) fn load_or_create(paths: &AppPaths) -> KandbResult<Self> {
        paths.ensure_dirs()?;
        let config_path = paths.config_file();

        let config = match fs::read_to_string(config_path) {
            Ok(content) => parse_config_file(config_path, &content)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save(paths)?;
                config
            }
            Err(source) => {
                return Err(KandbError::ReadConfigFile {
                    path: config_path.to_path_buf(),
                    source,
                });
            }
        };

        config.validate(paths)?;
        Ok(config)
    }

    pub(crate) fn save(&self, paths: &AppPaths) -> KandbResult<()> {
        paths.ensure_dirs()?;
        let content =
            toml::to_string_pretty(self).map_err(|source| KandbError::SerializeConfig {
                path: paths.config_file().to_path_buf(),
                message: source.to_string(),
            })?;

        fs::write(paths.config_file(), content).map_err(|source| KandbError::WriteConfigFile {
            path: paths.config_file().to_path_buf(),
            source,
        })
    }

    pub(crate) fn resolve_connections(
        &self,
        paths: &AppPaths,
    ) -> KandbResult<Vec<ResolvedConnectionProfile>> {
        self.connections
            .iter()
            .map(|connection| connection.resolve(paths.config_dir()))
            .collect()
    }

    fn validate(&self, paths: &AppPaths) -> KandbResult<()> {
        if self.version != CONFIG_VERSION {
            return Err(KandbError::UnsupportedConfigVersion {
                version: self.version,
            });
        }

        let mut ids = HashSet::new();
        for connection in &self.connections {
            if !ids.insert(connection.id.as_str()) {
                return Err(KandbError::DuplicateConnectionId(connection.id.clone()));
            }

            connection.resolve(paths.config_dir())?;
        }

        if let Some(default_id) = &self.default_connection_id
            && !self.connections.iter().any(|conn| conn.id == *default_id)
        {
            return Err(KandbError::MissingDefaultConnection(default_id.clone()));
        }

        Ok(())
    }
}

impl LoadedAppConfig {
    pub(crate) fn load() -> KandbResult<Self> {
        let paths = AppPaths::discover()?;
        let file = AppConfigFile::load_or_create(&paths)?;
        let resolved_connections = file.resolve_connections(&paths)?;

        Ok(Self {
            paths,
            file,
            resolved_connections,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct StoredConnectionProfile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider: String,
    #[serde(default)]
    pub(crate) config: toml::Table,
}

impl StoredConnectionProfile {
    fn resolve(&self, config_dir: &Path) -> KandbResult<ResolvedConnectionProfile> {
        let provider = match self.provider.as_str() {
            SQLITE_PROVIDER => {
                let config = parse_sqlite_config(&self.config, config_dir).map_err(|message| {
                    KandbError::ProviderConfigDecode {
                        connection_id: self.id.clone(),
                        provider: self.provider.clone(),
                        message,
                    }
                })?;
                ResolvedProviderConfig::Sqlite(config)
            }
            _ => ResolvedProviderConfig::Unknown {
                provider: self.provider.clone(),
                config: self.config.clone(),
            },
        };

        Ok(ResolvedConnectionProfile {
            id: self.id.clone(),
            name: self.name.clone(),
            provider,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedConnectionProfile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider: ResolvedProviderConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedProviderConfig {
    Sqlite(SqliteConfig),
    Unknown {
        provider: String,
        config: toml::Table,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredSqliteConfig {
    location: StoredSqliteLocation,
    #[serde(default)]
    read_only: bool,
    #[serde(default = "default_true")]
    create_if_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredSqliteLocation {
    Memory,
    Path { path: String },
    Uri { uri: String },
}

fn default_true() -> bool {
    true
}

fn parse_config_file(path: &Path, content: &str) -> KandbResult<AppConfigFile> {
    toml::from_str(content).map_err(|source| KandbError::ParseConfigFile {
        path: path.to_path_buf(),
        message: source.to_string(),
    })
}

fn parse_sqlite_config(table: &toml::Table, config_dir: &Path) -> Result<SqliteConfig, String> {
    let raw_value = toml::Value::Table(table.clone());
    let raw_config: StoredSqliteConfig = raw_value
        .try_into()
        .map_err(|err: toml::de::Error| err.to_string())?;

    let location = match raw_config.location {
        StoredSqliteLocation::Memory => SqliteLocation::Memory,
        StoredSqliteLocation::Path { path } => {
            SqliteLocation::Path(resolve_path(config_dir, &path).map_err(|err| err.to_string())?)
        }
        StoredSqliteLocation::Uri { uri } => SqliteLocation::Uri(uri),
    };

    Ok(SqliteConfig {
        location,
        read_only: raw_config.read_only,
        create_if_missing: raw_config.create_if_missing,
    })
}

fn resolve_path(config_dir: &Path, raw_path: &str) -> KandbResult<PathBuf> {
    if raw_path == "~" || raw_path.starts_with("~/") {
        let home_dir = dirs_next::home_dir().ok_or(KandbError::HomeDirNotAvailable)?;
        if raw_path == "~" {
            return Ok(home_dir);
        }

        return Ok(home_dir.join(
            raw_path
                .strip_prefix("~/")
                .expect("`~/` prefix must be present"),
        ));
    }

    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        return Ok(path);
    }

    Ok(config_dir.join(path))
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfigFile, CONFIG_FILE_NAME, ResolvedProviderConfig, StoredConnectionProfile,
        StoredSqliteConfig, StoredSqliteLocation,
    };
    use crate::app_paths::AppPaths;
    use kandb_provider_sqlite::SqliteLocation;
    use std::{fs, path::PathBuf};
    use tempfile::TempDir;

    #[test]
    fn load_or_create_writes_default_file() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));

        let config = AppConfigFile::load_or_create(&paths).expect("create default config");

        assert_eq!(config, AppConfigFile::default());
        assert!(paths.config_file().exists());
        assert!(paths.data_dir().exists());
    }

    #[test]
    fn invalid_toml_returns_error_without_rewriting_file() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));
        paths.ensure_dirs().expect("ensure dirs");
        fs::write(paths.config_file(), "version = {").expect("write invalid config");

        let error = AppConfigFile::load_or_create(&paths).expect_err("config should fail");
        let file_contents = fs::read_to_string(paths.config_file()).expect("read original file");

        assert!(error.to_string().contains("failed to parse config"));
        assert_eq!(file_contents, "version = {");
    }

    #[test]
    fn duplicate_connection_ids_are_rejected() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));
        let config = AppConfigFile {
            version: 1,
            default_connection_id: None,
            connections: vec![
                sqlite_profile("main", StoredSqliteLocation::Memory),
                sqlite_profile("main", StoredSqliteLocation::Memory),
            ],
        };

        config.save(&paths).expect("save config");

        let error = AppConfigFile::load_or_create(&paths).expect_err("duplicate id should fail");
        assert!(error.to_string().contains("duplicate connection id"));
    }

    #[test]
    fn missing_default_connection_is_rejected() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));
        let config = AppConfigFile {
            version: 1,
            default_connection_id: Some("missing".to_string()),
            connections: vec![sqlite_profile("main", StoredSqliteLocation::Memory)],
        };

        config.save(&paths).expect("save config");

        let error = AppConfigFile::load_or_create(&paths).expect_err("missing default should fail");
        assert!(error.to_string().contains("default connection"));
    }

    #[test]
    fn sqlite_config_resolves_memory_path_and_uri_locations() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));
        let relative = sqlite_profile(
            "relative",
            StoredSqliteLocation::Path {
                path: "db/main.sqlite".to_string(),
            },
        );
        let memory = sqlite_profile("memory", StoredSqliteLocation::Memory);
        let uri = sqlite_profile(
            "uri",
            StoredSqliteLocation::Uri {
                uri: "file:memdb1?mode=memory&cache=shared".to_string(),
            },
        );
        let config = AppConfigFile {
            version: 1,
            default_connection_id: Some("relative".to_string()),
            connections: vec![relative, memory, uri],
        };

        config.save(&paths).expect("save config");
        let loaded = AppConfigFile::load_or_create(&paths).expect("load config");
        let resolved = loaded
            .resolve_connections(&paths)
            .expect("resolve connections");

        assert!(matches!(
            &resolved[0].provider,
            ResolvedProviderConfig::Sqlite(config)
                if config.location == SqliteLocation::Path(paths.config_dir().join("db/main.sqlite"))
        ));
        assert!(matches!(
            &resolved[1].provider,
            ResolvedProviderConfig::Sqlite(config) if config.location == SqliteLocation::Memory
        ));
        assert!(matches!(
            &resolved[2].provider,
            ResolvedProviderConfig::Sqlite(config)
                if config.location
                    == SqliteLocation::Uri("file:memdb1?mode=memory&cache=shared".to_string())
        ));
    }

    #[test]
    fn unknown_provider_entries_are_preserved_on_save() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));
        let mut raw_config = toml::Table::new();
        raw_config.insert(
            "endpoint".to_string(),
            toml::Value::String("redis://127.0.0.1:6379".to_string()),
        );
        let config = AppConfigFile {
            version: 1,
            default_connection_id: Some("redis-local".to_string()),
            connections: vec![StoredConnectionProfile {
                id: "redis-local".to_string(),
                name: "Redis Local".to_string(),
                provider: "redis".to_string(),
                config: raw_config,
            }],
        };

        config.save(&paths).expect("save config");
        let loaded = AppConfigFile::load_or_create(&paths).expect("load config");
        let saved_text = fs::read_to_string(paths.config_file()).expect("read config file");

        assert_eq!(loaded, config);
        assert!(saved_text.contains("provider = \"redis\""));
        assert!(saved_text.contains("endpoint = \"redis://127.0.0.1:6379\""));
    }

    #[test]
    fn sqlite_home_prefixed_paths_expand_to_home_directory() {
        let temp_dir = TempDir::new().expect("temp dir");
        let paths =
            AppPaths::from_roots(temp_dir.path().join("config"), temp_dir.path().join("data"));
        let config = AppConfigFile {
            version: 1,
            default_connection_id: Some("main".to_string()),
            connections: vec![sqlite_profile(
                "main",
                StoredSqliteLocation::Path {
                    path: "~/db/main.sqlite".to_string(),
                },
            )],
        };

        let resolved = config
            .resolve_connections(&paths)
            .expect("resolve home-relative config");

        let expected_home = dirs_next::home_dir()
            .expect("home dir")
            .join("db/main.sqlite");
        assert!(matches!(
            &resolved[0].provider,
            ResolvedProviderConfig::Sqlite(sqlite)
                if sqlite.location == SqliteLocation::Path(expected_home)
        ));
    }

    fn sqlite_profile(id: &str, location: StoredSqliteLocation) -> StoredConnectionProfile {
        let config = StoredSqliteConfig {
            location,
            read_only: false,
            create_if_missing: false,
        };
        let value = toml::Value::try_from(&config).expect("serialize sqlite config");
        let table = value
            .as_table()
            .expect("sqlite config should serialize to a table")
            .clone();

        StoredConnectionProfile {
            id: id.to_string(),
            name: id.to_string(),
            provider: "sqlite".to_string(),
            config: table,
        }
    }

    #[test]
    fn config_file_name_is_stable() {
        assert_eq!(CONFIG_FILE_NAME, "config.toml");
        assert_eq!(
            PathBuf::from(CONFIG_FILE_NAME),
            PathBuf::from("config.toml")
        );
    }
}
