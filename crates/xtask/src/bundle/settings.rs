use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use kandb_i18n::macos_bundle_localizations;
use tauri_bundler::{AppCategory, BundleSettings, PackageSettings};

use crate::error::{Result, XtaskError};
use crate::manifest::{Manifest, resolve_manifest_path, resolve_manifest_paths};

pub fn read_bundle_settings(manifest_path: &Path) -> Result<(PackageSettings, BundleSettings)> {
    let manifest = Manifest::from_path(manifest_path)?;
    let manifest_dir = manifest_path.parent().ok_or_else(|| {
        XtaskError::msg(format!(
            "failed to resolve manifest dir for {}",
            manifest_path.display()
        ))
    })?;

    let bundle = manifest
        .package
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.bundle.as_ref());

    let product_name = bundle
        .and_then(|bundle| bundle.name.clone())
        .unwrap_or_else(|| manifest.package.name.clone());
    let description = manifest.package.description.clone().unwrap_or_default();

    let mut bundle_settings = BundleSettings::default();
    if let Some(bundle) = bundle {
        bundle_settings.identifier = bundle.identifier.clone();
        bundle_settings.publisher = bundle.publisher.clone().or_else(|| {
            bundle_settings
                .identifier
                .as_deref()
                .and_then(infer_publisher_from_identifier)
        });
        bundle_settings.icon = bundle
            .icon
            .as_ref()
            .map(|paths| resolve_manifest_paths(manifest_dir, paths));
        bundle_settings.category = bundle
            .category
            .as_deref()
            .map(parse_app_category)
            .transpose()?;
        bundle_settings.short_description = bundle.short_description.clone();
        bundle_settings.long_description = bundle.long_description.clone();
        bundle_settings.homepage = bundle
            .homepage
            .clone()
            .or_else(|| manifest.package.homepage.clone());
        bundle_settings.deep_link_protocols = bundle.deep_link_protocols.clone();
    }

    if bundle_settings.homepage.is_none() {
        bundle_settings.homepage = manifest.package.homepage.clone();
    }

    bundle_settings.license = manifest.package.license.clone();
    bundle_settings.license_file = manifest
        .package
        .license_file
        .as_deref()
        .map(|path| resolve_manifest_path(manifest_dir, path));
    bundle_settings.resources_map = Some(macos_bundle_localization_resources(manifest_dir)?);
    sync_windows_icon_path(&mut bundle_settings);

    let package_settings = PackageSettings {
        product_name,
        version: manifest.package.version.clone(),
        description,
        homepage: manifest
            .package
            .homepage
            .clone()
            .or_else(|| manifest.package.repository.clone()),
        authors: manifest.package.authors.clone(),
        default_run: manifest.package.default_run.clone(),
    };

    Ok((package_settings, bundle_settings))
}

#[allow(deprecated)]
fn sync_windows_icon_path(bundle_settings: &mut BundleSettings) {
    if bundle_settings.windows.icon_path != default_windows_icon_path() {
        return;
    }

    let Some(icon_path) = bundle_settings
        .icon
        .as_ref()
        .and_then(|paths| {
            paths.iter().find(|path| {
                Path::new(path)
                    .extension()
                    .and_then(OsStr::to_str)
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
            })
        })
        .map(PathBuf::from)
    else {
        return;
    };

    bundle_settings.windows.icon_path = icon_path;
}

fn default_windows_icon_path() -> PathBuf {
    PathBuf::from("icons/icon.ico")
}

fn macos_bundle_localization_resources(manifest_dir: &Path) -> Result<HashMap<String, String>> {
    let resources_root = fs::canonicalize(manifest_dir.join("../kandb-i18n/locales/macos"))
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to resolve macOS localization root relative to {}: {err}",
                manifest_dir.display()
            ))
        })?;
    let mut resources_map = HashMap::new();

    for localization in macos_bundle_localizations() {
        let lproj_dir = resources_root.join(localization.lproj_dir);
        if !lproj_dir.exists() {
            return Err(XtaskError::msg(format!(
                "missing macOS localization directory {}",
                lproj_dir.display()
            )));
        }

        for entry in walkdir::WalkDir::new(&lproj_dir) {
            let entry = entry.map_err(|err| {
                XtaskError::msg(format!(
                    "failed to walk macOS localization resources under {}: {err}",
                    lproj_dir.display()
                ))
            })?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = fs::canonicalize(entry.path()).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to resolve macOS localization resource {}: {err}",
                    entry.path().display()
                ))
            })?;
            let relative = path.strip_prefix(&resources_root).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to strip localization root {} from {}: {err}",
                    resources_root.display(),
                    path.display()
                ))
            })?;

            resources_map.insert(
                path.to_string_lossy().into_owned(),
                relative.to_string_lossy().into_owned(),
            );
        }
    }

    Ok(resources_map)
}

fn infer_publisher_from_identifier(identifier: &str) -> Option<String> {
    let mut parts = identifier.split('.');
    parts.next()?;
    parts
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_app_category(category: &str) -> Result<AppCategory> {
    category.parse().map_err(|suggestion| {
        let message = match suggestion {
            Some(suggestion) => {
                format!("invalid bundle category `{category}`, did you mean `{suggestion}`?")
            }
            None => format!("invalid bundle category `{category}`"),
        };
        XtaskError::msg(message)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "xtask-bundle-settings-{suffix}-{}",
                std::process::id()
            ));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[allow(deprecated)]
    #[test]
    fn read_bundle_settings_resolves_relative_bundle_paths() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let app_dir = temp_dir.path.join("crates/kandb");
        let i18n_dir = temp_dir.path.join("crates/kandb-i18n/locales/macos");
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(i18n_dir.join("en-US.lproj"))?;
        fs::create_dir_all(i18n_dir.join("zh-Hans.lproj"))?;
        fs::write(
            i18n_dir.join("en-US.lproj/InfoPlist.strings"),
            "\"CFBundleName\" = \"KanDB\";\n",
        )?;
        fs::write(
            i18n_dir.join("zh-Hans.lproj/InfoPlist.strings"),
            "\"CFBundleName\" = \"KanDB\";\n",
        )?;

        let manifest_path = app_dir.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "kandb"
version = "0.1.0"
license-file = "LICENSE"

[package.metadata.bundle]
name = "kanDB"
identifier = "top.sushao.kandb"
category = "DeveloperTool"
deep_link_protocols = [{ schemes = ["kandb"] }]
icon = [
  "../../assets/icon/app-icon.ico",
  "../../assets/icon.icon/Assets/app-icon.png",
  "../../assets/icon.icon",
]
"#,
        )?;

        let (_, bundle_settings) = read_bundle_settings(&manifest_path)?;
        let expected_ico = app_dir.join("../../assets/icon/app-icon.ico");
        let expected_png = app_dir.join("../../assets/icon.icon/Assets/app-icon.png");
        let expected_icon = app_dir.join("../../assets/icon.icon");
        assert_eq!(
            bundle_settings.icon,
            Some(vec![
                expected_ico.to_string_lossy().into_owned(),
                expected_png.to_string_lossy().into_owned(),
                expected_icon.to_string_lossy().into_owned(),
            ])
        );
        assert_eq!(bundle_settings.license_file, Some(app_dir.join("LICENSE")));
        assert_eq!(bundle_settings.category, Some(AppCategory::DeveloperTool));
        assert_eq!(
            bundle_settings
                .deep_link_protocols
                .as_ref()
                .expect("deep link protocols should be present")[0]
                .schemes,
            vec!["kandb".to_string()]
        );
        assert_eq!(bundle_settings.windows.icon_path, expected_ico);
        let resources_map = bundle_settings
            .resources_map
            .as_ref()
            .expect("macOS localization resources should be present");
        assert!(resources_map.iter().any(|(source, destination)| {
            source.ends_with("crates/kandb-i18n/locales/macos/en-US.lproj/InfoPlist.strings")
                && destination == "en-US.lproj/InfoPlist.strings"
        }));
        assert!(resources_map.iter().any(|(source, destination)| {
            source.ends_with("crates/kandb-i18n/locales/macos/zh-Hans.lproj/InfoPlist.strings")
                && destination == "zh-Hans.lproj/InfoPlist.strings"
        }));

        Ok(())
    }
}
