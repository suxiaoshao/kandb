use gpui::{
    App, FontWeight, Image, ImageFormat, IntoElement, ParentElement, Pixels, SharedString, Styled,
    div, img,
};
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use gpui_component::ActiveTheme;
use std::sync::{Arc, LazyLock};

const APP_ICON_BYTES: &[u8] =
    include_bytes!("../../../../assets/icon/app-icon.iconset/icon_128x128@2x.png");

static APP_ICON: LazyLock<Arc<Image>> =
    LazyLock::new(|| Arc::new(Image::from_bytes(ImageFormat::Png, APP_ICON_BYTES.to_vec())));

pub(crate) fn app_icon() -> Arc<Image> {
    APP_ICON.clone()
}

pub(crate) fn wordmark(size: Pixels, weight: FontWeight, cx: &App) -> impl IntoElement {
    let font_family = wordmark_font_family(cx);

    div()
        .text_size(size)
        .font_family(font_family)
        .font_weight(weight)
        .child("KanDB")
}

fn wordmark_font_family(_cx: &App) -> SharedString {
    #[cfg(target_os = "windows")]
    {
        return "Segoe UI".into();
    }

    #[cfg(target_os = "macos")]
    {
        ".SystemUIFont".into()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        _cx.theme().font_family.clone()
    }
}

pub(crate) fn logo_mark(size: Pixels) -> impl IntoElement {
    img(app_icon()).size(size).flex_none()
}
