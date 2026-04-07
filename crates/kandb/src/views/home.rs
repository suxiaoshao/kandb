mod sidebar_model;

use self::sidebar_model::{SidebarIcon, SidebarNodeKind, SidebarTree, VisibleSidebarNode};
use crate::{
    app_menus,
    components::provider_icon::provider_icon,
    config::LoadedAppConfig,
    i18n::I18n,
    workspace::{WorkspaceStore, save_now},
};
use gpui::{InteractiveElement as _, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Root, Sizable, Size, h_flex,
    label::Label,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement,
    v_flex,
};
use kandb_assets::{IconName, ProviderIconName};
use std::ops::Deref;

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
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let sidebar_focus_handle = cx.focus_handle();
        let workspace = cx.global::<WorkspaceStore>().deref().clone();
        let tree = SidebarTree::from_config(cx.global::<LoadedAppConfig>(), cx.global::<I18n>());
        let default_expanded_node_ids = tree.default_expanded_node_ids();
        let default_selected_node_id = tree.default_selected_node_id().map(ToOwned::to_owned);
        let valid_node_ids = tree.valid_node_ids();

        workspace.update(cx, |workspace, cx| {
            workspace.ensure_initial_sidebar_state(
                &valid_node_ids,
                default_selected_node_id.as_deref(),
                &default_expanded_node_ids,
                cx,
            );
        });

        Self {
            focus_handle,
            sidebar_focus_handle,
            _subscriptions: vec![
                cx.observe(&workspace, |_this, _, cx| {
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
        let tree = SidebarTree::from_config(cx.global::<LoadedAppConfig>(), cx.global::<I18n>());
        let workspace = cx.global::<WorkspaceStore>().deref().clone();

        workspace.update(cx, |workspace, cx| {
            let Some(selected_node_id) = workspace.selected_node_id() else {
                return;
            };
            let Some(selected_node) =
                tree.find_visible_node(workspace.expanded_node_ids(), selected_node_id)
            else {
                return;
            };

            if selected_node.expandable && selected_node.expanded {
                workspace.set_node_expanded(selected_node.id, false, cx);
                return;
            }

            if let Some(parent_id) = selected_node.parent_id {
                workspace.select_node(parent_id, cx);
            }
        });
    }

    fn move_right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        let tree = SidebarTree::from_config(cx.global::<LoadedAppConfig>(), cx.global::<I18n>());
        let workspace = cx.global::<WorkspaceStore>().deref().clone();

        workspace.update(cx, |workspace, cx| {
            let Some(selected_node_id) = workspace.selected_node_id() else {
                return;
            };
            let visible = tree.visible_nodes(workspace.expanded_node_ids());
            let Some(selected_index) = visible.iter().position(|node| node.id == selected_node_id)
            else {
                return;
            };
            let selected_node = &visible[selected_index];

            if !selected_node.expandable {
                return;
            }

            if !selected_node.expanded {
                workspace.set_node_expanded(selected_node.id.clone(), true, cx);
                return;
            }

            if let Some(next_node) = visible.get(selected_index + 1)
                && next_node.parent_id.as_deref() == Some(selected_node.id.as_str())
            {
                workspace.select_node(next_node.id.clone(), cx);
            }
        });
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let tree = SidebarTree::from_config(cx.global::<LoadedAppConfig>(), cx.global::<I18n>());
        let workspace = cx.global::<WorkspaceStore>().deref().clone();

        workspace.update(cx, |workspace, cx| {
            let visible = tree.visible_nodes(workspace.expanded_node_ids());
            if visible.is_empty() {
                return;
            }

            let next_index = tree
                .find_visible_index(workspace.expanded_node_ids(), workspace.selected_node_id())
                .map(|index| {
                    if delta.is_negative() {
                        index.saturating_sub(delta.unsigned_abs())
                    } else {
                        (index + delta as usize).min(visible.len().saturating_sub(1))
                    }
                })
                .unwrap_or(0);

            if let Some(next_node) = visible.get(next_index) {
                workspace.select_node(next_node.id.clone(), cx);
            }
        });
    }

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree = SidebarTree::from_config(cx.global::<LoadedAppConfig>(), cx.global::<I18n>());
        let workspace = cx.global::<WorkspaceStore>().read(cx);
        let selected_node_id = workspace.selected_node_id().map(ToOwned::to_owned);
        let expanded_node_ids = workspace.expanded_node_ids().clone();
        let visible_nodes = tree.visible_nodes(&expanded_node_ids);
        let sidebar_is_focused = self.sidebar_focus_handle.is_focused(window);
        let sidebar_focus_handle = self.sidebar_focus_handle.clone();

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
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Label::new(cx.global::<I18n>().t("home-sidebar-title"))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_2()
                    .gap_1()
                    .children(visible_nodes.into_iter().map(move |node| {
                        render_sidebar_row(
                            node,
                            selected_node_id.as_deref(),
                            sidebar_is_focused,
                            sidebar_focus_handle.clone(),
                            cx,
                        )
                    })),
            )
    }

    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let tree = SidebarTree::from_config(cx.global::<LoadedAppConfig>(), i18n);
        let workspace = cx.global::<WorkspaceStore>().read(cx);
        let selected_node = workspace
            .selected_node_id()
            .and_then(|selected| tree.find_visible_node(workspace.expanded_node_ids(), selected));

        let title = selected_node
            .as_ref()
            .map(|node| node.label.clone())
            .unwrap_or_else(|| i18n.t("home-empty-title").into());
        let subtitle = selected_node
            .as_ref()
            .map(|node| selected_node_description(node, i18n))
            .unwrap_or_else(|| i18n.t("home-empty-subtitle").into());

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(cx.theme().background)
            .child(
                div()
                    .w_full()
                    .max_w(px(520.0))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.35))
                    .p_6()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(render_selected_node_icon(selected_node.as_ref(), cx))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        Label::new(title)
                                            .text_size(px(22.0))
                                            .font_weight(FontWeight::SEMIBOLD),
                                    )
                                    .child(
                                        Label::new(subtitle)
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            ),
                    )
                    .child(
                        Label::new(i18n.t("home-placeholder-message"))
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
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
                        .child(resizable_panel().child(self.render_content(cx).into_any_element())),
                ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

fn render_sidebar_row(
    node: VisibleSidebarNode,
    selected_node_id: Option<&str>,
    sidebar_is_focused: bool,
    sidebar_focus_handle: FocusHandle,
    cx: &App,
) -> AnyElement {
    if matches!(node.icon, SidebarIcon::Provider(ProviderIconName::Sqlite)) {
        return provider_row(
            node,
            selected_node_id,
            sidebar_is_focused,
            sidebar_focus_handle,
            cx,
        );
    }

    let is_selected = selected_node_id == Some(node.id.as_str());
    let padding_left = px(10.0 + (node.depth as f32) * 16.0);
    let icon = match node.icon {
        SidebarIcon::Lucide(icon) => icon,
        SidebarIcon::Provider(_) => unreachable!("provider icons are handled above"),
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
        .child(render_disclosure(&node))
        .child(
            Icon::new(icon)
                .with_size(Size::Small)
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            Label::new(node.label.clone())
                .text_sm()
                .text_color(cx.theme().foreground),
        )
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            window.focus(&sidebar_focus_handle);
            cx.global::<WorkspaceStore>()
                .deref()
                .clone()
                .update(cx, |workspace, cx| {
                    workspace.select_node(node.id.clone(), cx)
                });
        })
        .into_any_element()
}

fn provider_row(
    node: VisibleSidebarNode,
    selected_node_id: Option<&str>,
    sidebar_is_focused: bool,
    sidebar_focus_handle: FocusHandle,
    cx: &App,
) -> AnyElement {
    let is_selected = selected_node_id == Some(node.id.as_str());
    let padding_left = px(10.0 + (node.depth as f32) * 16.0);

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
        .child(render_disclosure(&node))
        .child(provider_icon(ProviderIconName::Sqlite, px(16.0)))
        .child(
            Label::new(node.label.clone())
                .text_sm()
                .text_color(cx.theme().foreground),
        )
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            window.focus(&sidebar_focus_handle);
            cx.global::<WorkspaceStore>()
                .deref()
                .clone()
                .update(cx, |workspace, cx| {
                    workspace.select_node(node.id.clone(), cx)
                });
        })
        .into_any_element()
}

fn render_disclosure(node: &VisibleSidebarNode) -> impl IntoElement {
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
            this.cursor_pointer().on_click(move |_, _, cx| {
                cx.stop_propagation();
                cx.global::<WorkspaceStore>()
                    .deref()
                    .clone()
                    .update(cx, |workspace, cx| {
                        workspace.set_node_expanded(id.clone(), !expanded, cx);
                        workspace.select_node(id.clone(), cx);
                    });
            })
        })
}

fn selected_node_description(node: &VisibleSidebarNode, i18n: &I18n) -> SharedString {
    match node.kind {
        SidebarNodeKind::Connection => i18n.t("home-connection-description").into(),
        SidebarNodeKind::Namespace => i18n.t("home-namespace-description").into(),
        SidebarNodeKind::ResourceGroup => i18n.t("home-resource-group-description").into(),
    }
}

fn render_selected_node_icon(node: Option<&VisibleSidebarNode>, cx: &App) -> AnyElement {
    match node.map(|node| node.icon) {
        Some(SidebarIcon::Provider(icon)) => provider_icon(icon, px(24.0)),
        Some(SidebarIcon::Lucide(icon)) => Icon::new(icon)
            .with_size(Size::Large)
            .text_color(cx.theme().foreground)
            .into_any_element(),
        None => Icon::new(IconName::Server)
            .with_size(Size::Large)
            .text_color(cx.theme().foreground)
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::sidebar_model::SidebarTree;
    use crate::{
        app_paths::AppPaths,
        config::{
            AppConfigFile, LoadedAppConfig, ResolvedConnectionProfile, ResolvedProviderConfig,
            StoredConnectionProfile,
        },
    };
    use kandb_provider_sqlite::{SqliteConfig, SqliteLocation};
    use std::{collections::BTreeSet, path::PathBuf};

    fn sample_tree() -> SidebarTree {
        SidebarTree::from_config(
            &LoadedAppConfig {
            paths: AppPaths::from_roots(PathBuf::from("/tmp/config"), PathBuf::from("/tmp/data")),
            file: AppConfigFile {
                version: 1,
                default_connection_id: Some("local-main".to_owned()),
                connections: vec![StoredConnectionProfile {
                    id: "local-main".to_owned(),
                    name: "Local Main".to_owned(),
                    provider: "sqlite".to_owned(),
                    config: toml::Table::new(),
                }],
            },
            resolved_connections: vec![ResolvedConnectionProfile {
                id: "local-main".to_owned(),
                name: "Local Main".to_owned(),
                provider: ResolvedProviderConfig::Sqlite(SqliteConfig {
                    location: SqliteLocation::Memory,
                    read_only: false,
                    create_if_missing: true,
                }),
            }],
            },
            &crate::i18n::I18n::english_for_test(),
        )
    }

    #[test]
    fn sidebar_tree_has_default_expanded_connection_and_namespace() {
        let tree = sample_tree();
        let expanded = tree.default_expanded_node_ids();

        assert_eq!(
            expanded,
            BTreeSet::from([
                "connection:local-main".to_owned(),
                "namespace:local-main:main".to_owned(),
            ])
        );
    }

    #[test]
    fn selected_node_description_mentions_placeholder_groups() {
        let tree = sample_tree();
        let visible = tree.visible_nodes(&tree.default_expanded_node_ids());
        let description =
            super::selected_node_description(&visible[2], &crate::i18n::I18n::english_for_test());

        assert!(description.contains("Placeholder resource group"));
    }
}
