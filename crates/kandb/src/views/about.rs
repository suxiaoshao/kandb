#[cfg(not(target_os = "macos"))]
use crate::APP_TITLE;
use crate::{APP_ID, app_menus, i18n::I18n};
use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, TitlebarOptions, Window,
    WindowBounds, WindowKind, WindowOptions, px,
};
#[cfg(target_os = "macos")]
use gpui::{Point, point};
use gpui_component::{ActiveTheme, Sizable, button::Button, h_flex, label::Label, v_flex};
use kandb_i18n::FluentArgs;

pub(crate) fn open_about_window(cx: &mut App) {
    if let Some(existing) = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<AboutWindow>())
    {
        let _ = existing.update(cx, |view, window, _cx| {
            window.activate_window();
            view.focus_handle.focus(window);
        });
        return;
    }

    let window_size = about_window_size();

    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(window_size, cx)),
            titlebar: Some(about_titlebar_options(cx.global::<I18n>())),
            is_resizable: false,
            is_minimizable: false,
            kind: WindowKind::Normal,
            app_id: Some(APP_ID.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(AboutWindow::new);
            let focus_handle = view.read(cx).focus_handle.clone();
            window.activate_window();
            focus_handle.focus(window);
            view
        },
    );
}

fn about_window_size() -> gpui::Size<gpui::Pixels> {
    gpui::Size {
        width: px(320.),
        height: px(400.),
    }
}

#[cfg(target_os = "macos")]
fn about_titlebar_options(_i18n: &I18n) -> TitlebarOptions {
    TitlebarOptions {
        title: None,
        appears_transparent: true,
        traffic_light_position: Some(about_traffic_light_position()),
    }
}

#[cfg(not(target_os = "macos"))]
fn about_titlebar_options(i18n: &I18n) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(about_window_title(i18n).into()),
        ..Default::default()
    }
}

#[cfg(not(target_os = "macos"))]
fn about_window_title(i18n: &I18n) -> String {
    let mut args = FluentArgs::new();
    args.set("app_name", APP_TITLE);
    i18n.t_with_args("app-about-window-title", &args)
}

#[cfg(target_os = "macos")]
fn about_traffic_light_position() -> Point<gpui::Pixels> {
    point(px(12.), px(12.))
}

pub(crate) struct AboutWindow {
    focus_handle: FocusHandle,
    version: SharedString,
    description: SharedString,
    docs_url: SharedString,
    repository_url: SharedString,
}

impl AboutWindow {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: cx.global::<I18n>().t("app-about-description").into(),
            docs_url: env!("CARGO_PKG_HOMEPAGE").into(),
            repository_url: env!("CARGO_PKG_REPOSITORY").into(),
        }
    }

    fn close_window(
        &mut self,
        _: &app_menus::CloseWindow,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.remove_window();
    }

    fn minimize(&mut self, _: &app_menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &app_menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Focusable for AboutWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();

        v_flex()
            .id("about-window")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close_window))
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .size_full()
            .items_center()
            .justify_start()
            .gap_5()
            .pt_12()
            .pb_5()
            .px_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                v_flex()
                    .items_center()
                    .gap_4()
                    .child(crate::components::brand::logo_mark(px(58.)))
                    .child(crate::components::brand::wordmark(
                        px(20.),
                        FontWeight::SEMIBOLD,
                        cx,
                    ))
                    .child(
                        v_flex()
                            .items_center()
                            .gap_1()
                            .max_w(px(220.))
                            .child(
                                Label::new(self.description.clone())
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                Label::new(i18n.t("app-about-bootstrap-note"))
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        v_flex()
                            .items_center()
                            .gap_1()
                            .pt_2()
                            .child(
                                Label::new({
                                    let mut args = FluentArgs::new();
                                    args.set("version", self.version.as_ref());
                                    i18n.t_with_args("app-about-version", &args)
                                })
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD),
                            )
                            .child(
                                Label::new(i18n.t("app-about-roadmap-note"))
                                    .text_size(px(10.5))
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .pt_2()
                    .child(
                        Button::new("about-docs")
                            .label(i18n.t("app-about-docs"))
                            .small()
                            .on_click({
                                let docs_url = self.docs_url.clone();
                                move |_, _, cx: &mut App| cx.open_url(&docs_url)
                            }),
                    )
                    .child(
                        Button::new("about-github")
                            .label(i18n.t("app-about-github"))
                            .small()
                            .on_click({
                                let repository_url = self.repository_url.clone();
                                move |_, _, cx: &mut App| cx.open_url(&repository_url)
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_window_uses_compact_size() {
        let size = about_window_size();

        assert_eq!(size.width, px(320.));
        assert_eq!(size.height, px(400.));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_about_titlebar_is_transparent_with_visible_traffic_lights() {
        let titlebar = about_titlebar_options(&I18n::english_for_test());

        assert!(titlebar.title.is_none());
        assert!(titlebar.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            Some(about_traffic_light_position())
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_about_titlebar_keeps_system_title() {
        let titlebar = about_titlebar_options(&I18n::english_for_test());

        assert_eq!(
            titlebar.title.as_ref().map(|title| title.as_ref()),
            Some(format!("About {}", crate::APP_TITLE).as_str())
        );
        assert!(!titlebar.appears_transparent);
        assert!(titlebar.traffic_light_position.is_none());
    }
}
