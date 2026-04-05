pub(crate) mod about;
pub(crate) mod home;

use gpui::App;

pub(crate) fn init(cx: &mut App) {
    home::init(cx);
}
