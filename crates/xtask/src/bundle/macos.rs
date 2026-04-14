use kandb_i18n::macos_bundle_localizations;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use tauri_bundler::{BundleSettings, PlistKind};
use tracing::{info, warn};

use crate::error::{Result, XtaskError};

const ASSETS_CAR_DESTINATION: &str = "Assets.car";
const LIQUID_GLASS_ICON_NAME: &str = "Icon";

pub fn prepare_bundle_settings(out_dir: &Path, bundle_settings: &mut BundleSettings) -> Result<()> {
    let asset_source = select_liquid_glass_source(bundle_settings.icon.as_mut());
    let assets_car_path = match asset_source {
        Some(LiquidGlassSource::AssetsCar(path)) => Some(stage_assets_car(
            path.as_path(),
            &out_dir.join("liquid-glass"),
        )?),
        Some(LiquidGlassSource::IconComposer(path)) => Some(compile_icon_composer_asset(
            path.as_path(),
            &out_dir.join("liquid-glass"),
        )?),
        None => None,
    };

    if let Some(assets_car_path) = assets_car_path.as_ref() {
        bundle_settings
            .resources_map
            .get_or_insert_with(HashMap::new)
            .insert(
                assets_car_path.to_string_lossy().into_owned(),
                ASSETS_CAR_DESTINATION.to_string(),
            );
    }

    bundle_settings.macos.info_plist = Some(PlistKind::Plist(
        bundle_info_plist_overrides(assets_car_path.as_deref())?.into(),
    ));

    Ok(())
}

fn select_liquid_glass_source(icon_paths: Option<&mut Vec<String>>) -> Option<LiquidGlassSource> {
    let icon_paths = icon_paths?;
    let mut assets_car_path = None;
    let mut icon_composer_path = None;

    icon_paths.retain(|path| {
        let path_buf = PathBuf::from(path);
        match path_buf.extension() {
            Some(ext) if ext == OsStr::new("car") => {
                assets_car_path.get_or_insert(path_buf);
                false
            }
            Some(ext) if ext == OsStr::new("icon") => {
                icon_composer_path.get_or_insert(path_buf);
                false
            }
            _ => true,
        }
    });

    if icon_paths.is_empty() {
        *icon_paths = Vec::new();
    }

    assets_car_path
        .map(LiquidGlassSource::AssetsCar)
        .or_else(|| icon_composer_path.map(LiquidGlassSource::IconComposer))
}

fn stage_assets_car(source_path: &Path, staging_dir: &Path) -> Result<PathBuf> {
    if !source_path.is_file() {
        return Err(XtaskError::msg(format!(
            "{} must be an Assets.car file",
            source_path.display()
        )));
    }

    fs::create_dir_all(staging_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create liquid glass staging dir {}: {err}",
            staging_dir.display()
        ))
    })?;

    let destination = staging_dir.join(ASSETS_CAR_DESTINATION);
    fs::copy(source_path, &destination).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy {} to {}: {err}",
            source_path.display(),
            destination.display()
        ))
    })?;

    Ok(destination)
}

fn compile_icon_composer_asset(source_dir: &Path, staging_dir: &Path) -> Result<PathBuf> {
    if !source_dir.is_dir() {
        return Err(XtaskError::msg(format!(
            "{} must be an Icon Composer directory",
            source_dir.display()
        )));
    }

    let temp_dir = env::temp_dir().join(format!(
        "kandb-liquid-glass-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis()
    ));
    let icon_dest_path = temp_dir.join(format!("{LIQUID_GLASS_ICON_NAME}.icon"));
    let output_path = temp_dir.join("out");

    copy_dir(source_dir, &icon_dest_path)?;
    validate_icon_composer_asset(&icon_dest_path, &temp_dir)?;
    fs::create_dir_all(&output_path).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create actool output dir {}: {err}",
            output_path.display()
        ))
    })?;

    let actool_plist = output_path.join("assetcatalog_generated_info.plist");
    let output = Command::new("actool")
        .arg(&icon_dest_path)
        .arg("--compile")
        .arg(&output_path)
        .arg("--output-format")
        .arg("human-readable-text")
        .arg("--notices")
        .arg("--warnings")
        .arg("--output-partial-info-plist")
        .arg(&actool_plist)
        .arg("--app-icon")
        .arg(LIQUID_GLASS_ICON_NAME)
        .arg("--include-all-app-icons")
        .arg("--accent-color")
        .arg("AccentColor")
        .arg("--enable-on-demand-resources")
        .arg("NO")
        .arg("--development-region")
        .arg("en")
        .arg("--target-device")
        .arg("mac")
        .arg("--minimum-deployment-target")
        .arg("26.0")
        .arg("--platform")
        .arg("macosx")
        .output()
        .map_err(|err| XtaskError::CommandExecute {
            command: "actool".to_string(),
            source: err,
        })?;

    if !output.status.success() {
        let _ = fs::remove_dir_all(&temp_dir);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(XtaskError::msg(format!(
            "actool failed for {}: {}",
            source_dir.display(),
            stderr.trim()
        )));
    }

    let compiled_assets = output_path.join(ASSETS_CAR_DESTINATION);
    if !compiled_assets.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(XtaskError::msg(format!(
            "actool did not generate {} for {}",
            ASSETS_CAR_DESTINATION,
            source_dir.display()
        )));
    }

    let staged_assets = stage_assets_car(&compiled_assets, staging_dir)?;
    let _ = fs::remove_dir_all(&temp_dir);
    info!(path = %staged_assets.display(), "prepared liquid glass Assets.car");
    Ok(staged_assets)
}

fn validate_icon_composer_asset(icon_path: &Path, temp_dir: &Path) -> Result<()> {
    let preview_path = temp_dir.join("icon-preview.png");
    let output = Command::new(
        "/Applications/Xcode.app/Contents/Applications/Icon Composer.app/Contents/Executables/ictool",
    )
    .arg(icon_path)
    .arg("--export-image")
    .arg("--output-file")
    .arg(&preview_path)
    .arg("--platform")
    .arg("macOS")
    .arg("--rendition")
    .arg("Default")
    .arg("--width")
    .arg("256")
    .arg("--height")
    .arg("256")
    .arg("--scale")
    .arg("1")
    .output()
    .map_err(|err| XtaskError::CommandExecute {
        command: "ictool".to_string(),
        source: err,
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(XtaskError::msg(format!(
            "ictool failed for {}: {}",
            icon_path.display(),
            stderr.trim()
        )));
    }

    if !preview_path.exists() {
        return Err(XtaskError::msg(format!(
            "ictool did not generate a preview image for {}",
            icon_path.display()
        )));
    }

    Ok(())
}

fn copy_dir(source_dir: &Path, destination_dir: &Path) -> Result<()> {
    fs::create_dir_all(destination_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create destination dir {}: {err}",
            destination_dir.display()
        ))
    })?;

    for entry in walkdir::WalkDir::new(source_dir) {
        let entry = entry.map_err(|err| {
            XtaskError::msg(format!(
                "failed to walk Icon Composer dir {}: {err}",
                source_dir.display()
            ))
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(source_dir).map_err(|err| {
            XtaskError::msg(format!(
                "failed to strip source prefix {} from {}: {err}",
                source_dir.display(),
                path.display()
            ))
        })?;
        let destination = destination_dir.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to create directory {}: {err}",
                    destination.display()
                ))
            })?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    XtaskError::msg(format!(
                        "failed to create parent directory {}: {err}",
                        parent.display()
                    ))
                })?;
            }
            fs::copy(path, &destination).map_err(|err| {
                XtaskError::msg(format!(
                    "failed to copy {} to {}: {err}",
                    path.display(),
                    destination.display()
                ))
            })?;
        }
    }

    Ok(())
}

fn bundle_info_plist_overrides(assets_car_path: Option<&Path>) -> Result<plist::Dictionary> {
    let mut dict = plist::Dictionary::new();
    dict.insert(
        "CFBundleDevelopmentRegion".to_string(),
        plist::Value::String("en-US".to_string()),
    );
    dict.insert(
        "CFBundleLocalizations".to_string(),
        plist::Value::Array(
            macos_bundle_localizations()
                .iter()
                .map(|localization| {
                    plist::Value::String(localization.bundle_locale_tag.to_string())
                })
                .collect(),
        ),
    );

    if let Some(assets_car_path) = assets_car_path {
        if let Some(icon_name) = app_icon_name_from_assets_car(assets_car_path)? {
            dict.insert(
                "CFBundleIconName".to_string(),
                plist::Value::String(icon_name.clone()),
            );
            dict.insert(
                "CFBundleIconFile".to_string(),
                plist::Value::String(icon_name),
            );
        } else {
            warn!(
                path = %assets_car_path.display(),
                "failed to derive app icon name from Assets.car"
            );
        }
    }

    Ok(dict)
}

fn app_icon_name_from_assets_car(assets_car_path: &Path) -> Result<Option<String>> {
    let output = Command::new("assetutil")
        .arg("--info")
        .arg(assets_car_path)
        .output()
        .map_err(|err| XtaskError::CommandExecute {
            command: "assetutil".to_string(),
            source: err,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(XtaskError::msg(format!(
            "assetutil failed for {}: {}",
            assets_car_path.display(),
            stderr.trim()
        )));
    }

    let info: Vec<AssetsCarInfo> = serde_json::from_slice(&output.stdout)?;
    Ok(info
        .into_iter()
        .find(|entry| entry.asset_type == "Icon Image")
        .map(|entry| entry.name))
}

#[derive(Debug, Deserialize)]
struct AssetsCarInfo {
    #[serde(rename = "AssetType", default)]
    asset_type: String,
    #[serde(rename = "Name", default)]
    name: String,
}

#[derive(Debug, PartialEq, Eq)]
enum LiquidGlassSource {
    AssetsCar(PathBuf),
    IconComposer(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::{LiquidGlassSource, bundle_info_plist_overrides, select_liquid_glass_source};
    use crate::error::Result;

    #[test]
    fn selects_assets_car_before_icon_composer_and_filters_special_inputs() {
        let mut icons = vec![
            "/tmp/app-icon.ico".to_string(),
            "/tmp/app-icon.png".to_string(),
            "/tmp/LiquidGlass.icon".to_string(),
            "/tmp/Assets.car".to_string(),
        ];

        let source = select_liquid_glass_source(Some(&mut icons));

        assert_eq!(
            source,
            Some(LiquidGlassSource::AssetsCar("/tmp/Assets.car".into()))
        );
        assert_eq!(
            icons,
            vec![
                "/tmp/app-icon.ico".to_string(),
                "/tmp/app-icon.png".to_string()
            ]
        );
    }

    #[test]
    fn plist_overrides_always_include_bundle_localizations() -> Result<()> {
        let dict = bundle_info_plist_overrides(None)?;

        assert_eq!(
            dict.get("CFBundleDevelopmentRegion"),
            Some(&plist::Value::String("en-US".to_string()))
        );
        assert_eq!(
            dict.get("CFBundleLocalizations"),
            Some(&plist::Value::Array(vec![
                plist::Value::String("en-US".to_string()),
                plist::Value::String("zh-Hans".to_string()),
            ]))
        );

        Ok(())
    }
}
