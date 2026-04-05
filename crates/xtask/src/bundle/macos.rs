use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};

use crate::cmd::{command_exists, run_cmd_os};
use crate::error::{Result, XtaskError};

pub fn first_app_bundle(bundle_dir: &Path) -> Result<Option<PathBuf>> {
    for bundle_subdir in ["macos", "osx"] {
        let app_bundle_dir = bundle_dir.join(bundle_subdir);
        if let Some(app_path) = first_app_bundle_in_dir(&app_bundle_dir)? {
            return Ok(Some(app_path));
        }
    }

    Ok(None)
}

fn first_app_bundle_in_dir(app_bundle_dir: &Path) -> Result<Option<PathBuf>> {
    if !app_bundle_dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(app_bundle_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read {}: {err}",
            app_bundle_dir.display()
        ))
    })? {
        let path = entry
            .map_err(|err| {
                XtaskError::msg(format!(
                    "failed to read entry under {}: {err}",
                    app_bundle_dir.display()
                ))
            })?
            .path();
        if path.is_dir() && path.extension().and_then(OsStr::to_str) == Some("app") {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

pub fn inject_liquid_glass_icon(app_dir: &Path, app_path: &Path) -> Result<()> {
    let icon_dir = app_dir.join("../../assets/icon.icon");
    let icon_name = icon_dir
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Icon");

    if !icon_dir.exists() {
        warn!(icon_dir = %icon_dir.display(), "no .icon directory found, skipping Liquid Glass icon injection");
        return Ok(());
    }

    if !command_exists("xcrun") {
        warn!("xcrun not found, skipping Liquid Glass icon injection");
        return Ok(());
    }

    let tmp_dir = env::temp_dir().join(format!(
        "kandb-assets-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| XtaskError::msg(format!("failed to read system time: {err}")))?
            .as_millis()
    ));
    fs::create_dir_all(&tmp_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create temp dir {}: {err}",
            tmp_dir.display()
        ))
    })?;

    let actool_plist = tmp_dir.join("assetcatalog_generated_info.plist");
    let actool_args: Vec<&OsStr> = vec![
        OsStr::new("actool"),
        icon_dir.as_os_str(),
        OsStr::new("--compile"),
        tmp_dir.as_os_str(),
        OsStr::new("--output-format"),
        OsStr::new("human-readable-text"),
        OsStr::new("--notices"),
        OsStr::new("--warnings"),
        OsStr::new("--errors"),
        OsStr::new("--output-partial-info-plist"),
        actool_plist.as_os_str(),
        OsStr::new("--app-icon"),
        OsStr::new(icon_name),
        OsStr::new("--include-all-app-icons"),
        OsStr::new("--enable-on-demand-resources"),
        OsStr::new("NO"),
        OsStr::new("--development-region"),
        OsStr::new("en"),
        OsStr::new("--target-device"),
        OsStr::new("mac"),
        OsStr::new("--platform"),
        OsStr::new("macosx"),
        OsStr::new("--minimum-deployment-target"),
        OsStr::new("26.0"),
    ];

    let actool_result = run_cmd_os("xcrun", &actool_args, None);
    if let Err(err) = actool_result {
        let _ = fs::remove_dir_all(&tmp_dir);
        warn!(error = %err, "actool failed, skipping Liquid Glass icon injection");
        return Ok(());
    }

    let assets_car = tmp_dir.join("Assets.car");
    if !assets_car.exists() {
        let _ = fs::remove_dir_all(&tmp_dir);
        warn!("Assets.car was not generated, skipping Liquid Glass icon injection");
        return Ok(());
    }

    let target_assets = app_path.join("Contents/Resources/Assets.car");
    fs::copy(&assets_car, &target_assets).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy {} to {}: {err}",
            assets_car.display(),
            target_assets.display()
        ))
    })?;

    let plist = app_path.join("Contents/Info.plist");
    update_bundle_icon_name(&plist, icon_name)?;

    if command_exists("codesign") {
        let codesign_args: Vec<&OsStr> = vec![
            OsStr::new("--force"),
            OsStr::new("--deep"),
            OsStr::new("--sign"),
            OsStr::new("-"),
            app_path.as_os_str(),
        ];
        run_cmd_os("codesign", &codesign_args, None)?;
    }

    let _ = fs::remove_dir_all(&tmp_dir);
    info!(app_path = %app_path.display(), "injected Liquid Glass icon");
    Ok(())
}

fn update_bundle_icon_name(plist_path: &Path, icon_name: &str) -> Result<()> {
    let mut value = plist::Value::from_file(plist_path)?;
    let dict = value.as_dictionary_mut().ok_or_else(|| {
        XtaskError::msg(format!(
            "unexpected plist root type for {}: expected dictionary",
            plist_path.display()
        ))
    })?;
    dict.insert(
        "CFBundleIconName".to_string(),
        plist::Value::String(icon_name.to_string()),
    );
    value.to_file_xml(plist_path)?;
    Ok(())
}
