use gpui::{AnyElement, IntoElement, Pixels, Styled, img};
use kandb_assets::ProviderIconName;

pub(crate) fn provider_icon(icon: ProviderIconName, size: Pixels) -> AnyElement {
    let path = match icon {
        ProviderIconName::Sqlite => "icons/providers/sqlite.svg",
    };

    img(path).size(size).flex_none().into_any_element()
}
