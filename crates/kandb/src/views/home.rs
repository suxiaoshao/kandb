pub(crate) mod sidebar_model;
mod sidebar_state;

use self::{
    sidebar_model::{SidebarIcon, SidebarTree, VisibleSidebarNode},
    sidebar_state::SidebarState,
};
use crate::{
    app_menus,
    components::provider_icon::provider_icon,
    config::LoadedAppConfig,
    i18n::I18n,
    workspace::{WorkspaceStore, save_now},
};
use gpui::{InteractiveElement as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Root, Sizable, Size, h_flex,
    button::{Button, ButtonVariants},
    label::Label,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement,
    v_flex,
};
use kandb_assets::{IconName, ProviderIconName};
use std::{collections::BTreeSet, ops::Deref};

const SIDEBAR_CONTEXT: &str = "HomeSidebar";

actions!(home_sidebar, [MoveUp, MoveDown, MoveLeft, MoveRight]);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", MoveUp, Some(SIDEBAR_CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(SIDEBAR_CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(SIDEBAR_CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(SIDEBAR_CONTEXT)),
    ]);
}

pub(crate) struct HomeView {
    focus_handle: FocusHandle,
    sidebar_focus_handle: FocusHandle,
    sidebar_state: Entity<SidebarState>,
    selected_node_id: Option<String>,
    expanded_node_ids: BTreeSet<String>,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let sidebar_focus_handle = cx.focus_handle();
        let config = cx.global::<LoadedAppConfig>().clone();
        let sidebar_state = cx.new(|_| SidebarState::from_config(&config.resolved_connections));

        let mut this = Self {
            focus_handle,
            sidebar_focus_handle,
            sidebar_state: sidebar_state.clone(),
            selected_node_id: None,
            expanded_node_ids: BTreeSet::new(),
            _subscriptions: vec![
                cx.observe(&sidebar_state, |this, _, cx| {
                    this.reconcile_sidebar(cx);
                    cx.notify();
                }),
                cx.observe_window_bounds(window, |this, window, cx| {
                    this.sync_window_bounds(window, cx);
                }),
                cx.on_app_quit(|_this, cx| {
                    save_now(cx);
                    async {}
                }),
            ],
        };

        this.reconcile_sidebar(cx);
        this
    }

    fn reconcile_sidebar(&mut self, cx: &mut Context<Self>) {
        let locale = cx.global::<I18n>().locale_tag().to_string();
        self.sidebar_state
            .update(cx, |state, cx| state.preload_all_connections(&locale, cx));

        let valid_node_ids = self.tree(cx).valid_node_ids();
        self.expanded_node_ids
            .retain(|node_id| valid_node_ids.contains(node_id));
        if self
            .selected_node_id
            .as_ref()
            .is_some_and(|node_id| !valid_node_ids.contains(node_id))
        {
            self.selected_node_id = None;
        }
    }

    fn tree(&self, cx: &App) -> SidebarTree {
        self.sidebar_state.read(cx).build_tree(cx.global::<I18n>())
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

    fn sync_window_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = window.window_bounds();
        cx.global::<WorkspaceStore>()
            .deref()
            .clone()
            .update(cx, |workspace, cx| workspace.set_window_bounds(bounds, cx));
    }

    fn move_up(&mut self, _: &MoveUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(-1, cx);
    }

    fn move_down(&mut self, _: &MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    fn move_left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        let tree = self.tree(cx);
        let Some(selected_node_id) = self.selected_node_id.as_deref() else {
            return;
        };
        let Some(selected_node) = tree.find_visible_node(&self.expanded_node_ids, selected_node_id)
        else {
            return;
        };

        if selected_node.expandable && selected_node.expanded {
            self.expanded_node_ids.remove(selected_node.id.as_str());
            cx.notify();
            return;
        }

        if let Some(parent_id) = selected_node.parent_id {
            self.selected_node_id = Some(parent_id);
            cx.notify();
        }
    }

    fn move_right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        let tree = self.tree(cx);
        let Some(selected_node_id) = self.selected_node_id.as_deref() else {
            return;
        };
        let visible = tree.visible_nodes(&self.expanded_node_ids);
        let Some(selected_index) = visible.iter().position(|node| node.id == selected_node_id)
        else {
            return;
        };
        let selected_node = &visible[selected_index];

        if !selected_node.expandable {
            return;
        }

        if !selected_node.expanded {
            self.expanded_node_ids.insert(selected_node.id.clone());
            cx.notify();
            return;
        }

        if let Some(next_node) = visible.get(selected_index + 1)
            && next_node.parent_id.as_deref() == Some(selected_node.id.as_str())
            && next_node.selectable
        {
            self.selected_node_id = Some(next_node.id.clone());
            cx.notify();
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let tree = self.tree(cx);
        let visible = tree.visible_nodes(&self.expanded_node_ids);
        if visible.is_empty() {
            return;
        }

        let next_index = tree
            .find_visible_index(&self.expanded_node_ids, self.selected_node_id.as_deref())
            .map(|index| {
                let mut candidate = if delta.is_negative() {
                    index.saturating_sub(delta.unsigned_abs())
                } else {
                    (index + delta as usize).min(visible.len().saturating_sub(1))
                };

                while candidate < visible.len() && !visible[candidate].selectable {
                    if delta.is_negative() {
                        if candidate == 0 {
                            break;
                        }
                        candidate -= 1;
                    } else {
                        candidate += 1;
                        if candidate >= visible.len() {
                            candidate = visible.len().saturating_sub(1);
                            break;
                        }
                    }
                }

                candidate
            })
            .unwrap_or(0);

        if let Some(next_node) = visible.get(next_index)
            && next_node.selectable
        {
            self.selected_node_id = Some(next_node.id.clone());
            cx.notify();
        }
    }

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree = self.tree(cx);
        let selected_node_id = self.selected_node_id.clone();
        let visible_nodes = tree.visible_nodes(&self.expanded_node_ids);
        let selected_connection_node_id = selected_node_id
            .as_deref()
            .and_then(|node_id| tree.connection_node_id_for(node_id).map(ToOwned::to_owned));
        let refresh_loading = selected_connection_node_id
            .as_deref()
            .map(|connection_id| {
                self.sidebar_state
                    .read(cx)
                    .is_connection_refreshing(connection_id)
            })
            .unwrap_or_else(|| self.sidebar_state.read(cx).is_any_refreshing());
        let sidebar_is_focused = self.sidebar_focus_handle.is_focused(window);
        let locale = cx.global::<I18n>().locale_tag().to_string();
        let i18n = cx.global::<I18n>();
        let refresh_tooltip = selected_connection_node_id
            .as_ref()
            .map(|_| i18n.t("home-sidebar-refresh-connection"))
            .unwrap_or_else(|| i18n.t("home-sidebar-refresh-all"));
        let delete_tooltip = i18n.t("home-sidebar-delete-select-connection");
        let add_tooltip = i18n.t("home-sidebar-add-connection");

        div()
            .key_context(SIDEBAR_CONTEXT)
            .track_focus(&self.sidebar_focus_handle)
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::move_down))
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("home-sidebar-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .tooltip(refresh_tooltip)
                            .loading(refresh_loading)
                            .on_click({
                                let sidebar_state = self.sidebar_state.clone();
                                let target = selected_connection_node_id.clone();
                                move |_, _, cx| {
                                    sidebar_state.update(cx, |state, cx| {
                                        if let Some(connection_node_id) = target.as_deref() {
                                            state.refresh_connection(connection_node_id, &locale, cx);
                                        } else {
                                            state.refresh_all_connections(&locale, cx);
                                        }
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("home-sidebar-delete")
                            .ghost()
                            .small()
                            .icon(IconName::Trash2)
                            .tooltip(delete_tooltip)
                            .disabled(true)
                            .on_click(|_, _, _| {}),
                    )
                    .child(
                        Button::new("home-sidebar-add")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .tooltip(add_tooltip)
                            .on_click(|_, _, _| {}),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_2()
                    .gap_1()
                    .children(visible_nodes.into_iter().map(|node| {
                        self.render_sidebar_row(node, selected_node_id.as_deref(), sidebar_is_focused, cx)
                    })),
            )
    }

    fn render_sidebar_row(
        &self,
        node: VisibleSidebarNode,
        selected_node_id: Option<&str>,
        sidebar_is_focused: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let is_selected = selected_node_id == Some(node.id.as_str());
        let padding_left = px(10.0 + (node.depth as f32) * 16.0);
        let icon = match node.icon {
            SidebarIcon::Folder => {
                if node.expanded {
                    SidebarIcon::Lucide(IconName::FolderOpen)
                } else {
                    SidebarIcon::Lucide(IconName::FolderClosed)
                }
            }
            icon => icon,
        };

        h_flex()
            .id(SharedString::from(node.id.clone()))
            .w_full()
            .items_center()
            .gap_2()
            .rounded(px(8.0))
            .border_1()
            .border_color(cx.theme().transparent)
            .px_2()
            .py_1p5()
            .pl(padding_left)
            .when(is_selected, |this| {
                this.bg(gpui::hsla(
                    214.0 / 360.0,
                    0.58,
                    0.50,
                    if sidebar_is_focused { 0.18 } else { 0.10 },
                ))
                .border_color(gpui::hsla(
                    214.0 / 360.0,
                    0.58,
                    0.50,
                    if sidebar_is_focused { 0.55 } else { 0.24 },
                ))
            })
            .when(!is_selected, |this| {
                this.hover(|style| style.bg(gpui::hsla(214.0 / 360.0, 0.18, 0.48, 0.08)))
            })
            .child(div().flex_none().child(self.render_disclosure(&node, cx)))
            .child(div().flex_none().child(render_icon(icon, cx)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .child(
                        Label::new(node.label.clone())
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    ),
            )
            .when(node.selectable, |this| {
                let node_id = node.id.clone();
                this.cursor_pointer().on_click(cx.listener(move |this, _, window, cx| {
                    window.focus(&this.sidebar_focus_handle);
                    this.selected_node_id = Some(node_id.clone());
                    cx.notify();
                }))
            })
            .into_any_element()
    }

    fn render_disclosure(&self, node: &VisibleSidebarNode, cx: &Context<Self>) -> impl IntoElement {
        let id = node.id.clone();
        let expandable = node.expandable;
        let expanded = node.expanded;

        div()
            .id(SharedString::from(format!("toggle-{}", node.id)))
            .size(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .child(if expandable {
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .with_size(Size::XSmall)
                .into_any_element()
            } else {
                div().size(px(12.0)).into_any_element()
            })
            .when(expandable, |this| {
                this.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if expanded {
                        this.expanded_node_ids.remove(id.as_str());
                    } else {
                        this.expanded_node_ids.insert(id.clone());
                    }
                    this.selected_node_id = Some(id.clone());
                    cx.notify();
                }))
            })
    }
}

impl Focusable for HomeView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.sidebar_focus_handle.clone()
    }
}

impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let sidebar_width = cx.global::<WorkspaceStore>().read(cx).sidebar_width();

        v_flex()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close_window))
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .size_full()
            .bg(cx.theme().background)
            .child(
                div().flex_1().overflow_hidden().child(
                    h_resizable("home-layout")
                        .on_resize(|state, _window, cx| {
                            let width =
                                state.read(cx).sizes().first().copied().unwrap_or(px(280.0));
                            cx.global::<WorkspaceStore>()
                                .deref()
                                .clone()
                                .update(cx, |workspace, cx| workspace.set_sidebar_width(width, cx));
                        })
                        .child(
                            resizable_panel()
                                .size(sidebar_width)
                                .size_range(px(220.0)..Pixels::MAX)
                                .child(self.render_sidebar(window, cx)),
                        )
                        .child(
                            resizable_panel().child(
                                div()
                                    .size_full()
                                    .bg(cx.theme().background)
                                    .into_any_element(),
                            ),
                        ),
                ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

fn render_icon(icon: SidebarIcon, cx: &App) -> AnyElement {
    match icon {
        SidebarIcon::Provider(ProviderIconName::Sqlite) => {
            provider_icon(ProviderIconName::Sqlite, px(16.0))
        }
        SidebarIcon::Lucide(icon) => Icon::new(icon)
            .with_size(Size::Small)
            .text_color(cx.theme().muted_foreground)
            .into_any_element(),
        SidebarIcon::Folder => Icon::new(IconName::FolderClosed)
            .with_size(Size::Small)
            .text_color(cx.theme().muted_foreground)
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::{SidebarIcon, SidebarTree};
    use crate::views::home::sidebar_model::{
        SidebarChildren, SidebarNode, connection_node_id, provider_node_id,
    };
    use kandb_assets::{IconName, ProviderIconName};
    use std::collections::BTreeSet;

    fn sample_tree() -> SidebarTree {
        SidebarTree::new(vec![SidebarNode {
            id: connection_node_id("local"),
            label: "Local".into(),
            icon: SidebarIcon::Provider(ProviderIconName::Sqlite),
            parent_id: None,
            selectable: true,
            children: SidebarChildren::Branch(vec![SidebarNode {
                id: provider_node_id("local", "namespace:main"),
                label: "main".into(),
                icon: SidebarIcon::Lucide(IconName::HardDrive),
                parent_id: Some(connection_node_id("local")),
                selectable: true,
                children: SidebarChildren::Leaf,
            }]),
        }])
    }

    #[test]
    fn connection_lookup_still_works_for_nested_nodes() {
        let tree = sample_tree();
        let nested = provider_node_id("local", "namespace:main");
        let connection = connection_node_id("local");

        assert_eq!(tree.connection_node_id_for(&nested), Some(connection.as_str()));
    }

    #[test]
    fn visible_tree_respects_expanded_state() {
        let tree = sample_tree();
        let collapsed = tree.visible_nodes(&BTreeSet::new());
        let expanded = tree.visible_nodes(&BTreeSet::from([connection_node_id("local")]));

        assert_eq!(collapsed.len(), 1);
        assert_eq!(expanded.len(), 2);
    }
}
