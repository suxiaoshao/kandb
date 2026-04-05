use image::{GenericImageView, ImageDecoder};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::error::{Result, XtaskError};

pub(crate) fn prepare_bundle_icons(app_dir: &Path) -> Result<()> {
    let workspace_assets = app_dir.join("../../assets");
    let src_png = workspace_assets.join("icon.icon/Assets/app-icon.png");
    if !src_png.exists() {
        return Err(XtaskError::msg(format!(
            "missing source icon {}",
            src_png.display()
        )));
    }

    let iconset_dir = workspace_assets.join("icon/app-icon.iconset");
    let required_icon = iconset_dir.join("icon_512x512@2x.png");
    let should_regenerate = !required_icon.exists() || is_rgba16_png(&required_icon)?;

    if !should_regenerate {
        return Ok(());
    }

    fs::create_dir_all(&iconset_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create iconset dir {}: {err}",
            iconset_dir.display()
        ))
    })?;

    for entry in fs::read_dir(&iconset_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to read iconset dir {}: {err}",
            iconset_dir.display()
        ))
    })? {
        let path = entry
            .map_err(|err| XtaskError::msg(format!("failed to read iconset dir entry: {err}")))?
            .path();
        if path.extension().and_then(OsStr::to_str) == Some("png") {
            fs::remove_file(&path).map_err(|err| {
                XtaskError::msg(format!("failed to remove {}: {err}", path.display()))
            })?;
        }
    }

    let source_image = image::ImageReader::open(&src_png)
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to open source icon {}: {err}",
                src_png.display()
            ))
        })?
        .decode()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to decode source icon {}: {err}",
                src_png.display()
            ))
        })?;

    let ico_path = workspace_assets.join("icon/app-icon.ico");
    source_image
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .save(&ico_path)
        .map_err(|err| XtaskError::msg(format!("failed to save {}: {err}", ico_path.display())))?;

    for size in [16_u32, 32, 128, 256, 512] {
        let base = format!("icon_{size}x{size}.png");
        let retina = format!("icon_{size}x{size}@2x.png");

        let base_image =
            source_image.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        base_image
            .save(iconset_dir.join(base))
            .map_err(|err| XtaskError::msg(format!("failed to save iconset image: {err}")))?;

        let doubled = size * 2;
        let retina_image =
            source_image.resize_exact(doubled, doubled, image::imageops::FilterType::Lanczos3);
        retina_image
            .save(iconset_dir.join(retina))
            .map_err(|err| XtaskError::msg(format!("failed to save iconset image: {err}")))?;
    }

    let liquid_asset_path = workspace_assets.join("icon.icon/Assets/app-icon.png");
    if liquid_asset_path.exists() {
        let icon_asset = image::ImageReader::open(&liquid_asset_path)
            .map_err(|err| {
                XtaskError::msg(format!(
                    "failed to open {}: {err}",
                    liquid_asset_path.display()
                ))
            })?
            .decode()
            .map_err(|err| {
                XtaskError::msg(format!(
                    "failed to decode {}: {err}",
                    liquid_asset_path.display()
                ))
            })?;

        if icon_asset.dimensions() != (1024, 1024) {
            icon_asset
                .resize_exact(1024, 1024, image::imageops::FilterType::Lanczos3)
                .save(&liquid_asset_path)
                .map_err(|err| {
                    XtaskError::msg(format!(
                        "failed to normalize {}: {err}",
                        liquid_asset_path.display()
                    ))
                })?;
        }
    }

    Ok(())
}

fn is_rgba16_png(path: &Path) -> Result<bool> {
    let file = fs::File::open(path)
        .map_err(|err| XtaskError::msg(format!("failed to open {}: {err}", path.display())))?;
    let reader = std::io::BufReader::new(file);
    let decoder = image::codecs::png::PngDecoder::new(reader)
        .map_err(|err| XtaskError::msg(format!("failed to parse png {}: {err}", path.display())))?;

    Ok(decoder.original_color_type() == image::ExtendedColorType::Rgba16)
}
