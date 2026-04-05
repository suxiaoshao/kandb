use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::error::{Result, XtaskError};

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub package: ManifestPackage,
    #[serde(default)]
    pub bin: Vec<ManifestBinary>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestBinary {
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestPackage {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    #[serde(rename = "default-run", default)]
    pub default_run: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(rename = "license-file", default)]
    pub license_file: Option<String>,
    #[serde(default)]
    pub metadata: Option<ManifestMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestMetadata {
    #[serde(default)]
    pub bundle: Option<ManifestBundle>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestBundle {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub identifier: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub icon: Option<Vec<String>>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub short_description: Option<String>,
    #[serde(default)]
    pub long_description: Option<String>,
    #[serde(default)]
    pub deep_link_protocols: Option<Vec<tauri_utils::config::DeepLinkProtocol>>,
}

impl Manifest {
    pub fn from_path(manifest_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(manifest_path).map_err(|err| {
            XtaskError::msg(format!("failed to read {}: {err}", manifest_path.display()))
        })?;

        toml::from_str(&content).map_err(Into::into)
    }

    pub fn main_binary_name(&self) -> String {
        self.bin
            .iter()
            .find(|bin| bin.path.as_deref() == Some("src/main.rs"))
            .map(|bin| bin.name.clone())
            .or_else(|| self.package.default_run.clone())
            .unwrap_or_else(|| self.package.name.clone())
    }
}

pub fn resolve_manifest_path(manifest_dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

pub fn resolve_manifest_paths(manifest_dir: &Path, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| resolve_manifest_path(manifest_dir, path))
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}
