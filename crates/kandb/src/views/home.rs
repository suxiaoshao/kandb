use gpui::*;
use gpui_component::{ActiveTheme, Root, TitleBar, label::Label, v_flex};

pub(crate) fn init(_cx: &mut App) {}

pub(crate) struct HomeView;

impl HomeView {
    pub(crate) fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }

    fn close_window(
        &mut self,
        _: &crate::app_menus::CloseWindow,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.remove_window();
    }

    fn minimize(
        &mut self,
        _: &crate::app_menus::Minimize,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &crate::app_menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        v_flex()
            .on_action(cx.listener(Self::close_window))
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .size_full()
            .bg(cx.theme().background)
            .child(div().child(TitleBar::new()).flex_initial())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .px_6()
                    .child(crate::components::brand::wordmark(
                        px(30.),
                        gpui::FontWeight::SEMIBOLD,
                        cx,
                    ))
                    .child(
                        Label::new(
                            "A focused desktop database client, currently in KanDB bootstrap.",
                        )
                        .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new("Home is intentionally empty for now.")
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}
