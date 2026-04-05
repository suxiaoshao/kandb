use gpui::*;
use gpui_component::{ActiveTheme, Root, TitleBar, label::Label, v_flex};

pub(crate) fn init(_cx: &mut App) {}

pub(crate) struct HomeView;

impl HomeView {
    pub(crate) fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        v_flex()
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
                    .child(Label::new("kanDB").text_size(px(28.)))
                    .child(
                        Label::new("A focused desktop database client, currently in bootstrap.")
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
