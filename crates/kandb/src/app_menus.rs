use crate::{APP_TITLE, i18n::I18n, views::about::open_about_window};
use fluent_bundle::FluentArgs;
#[cfg(target_os = "macos")]
use gpui::SystemMenuType;
use gpui::{App, KeyBinding, Menu, MenuItem, actions};
use tracing::{Level, event};

actions!(
    kandb,
    [
        About,
        Quit,
        CloseWindow,
        Minimize,
        Zoom,
        Hide,
        HideOthers,
        ShowAll
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
    ]);

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
    ]);

    cx.on_action(|_: &About, cx: &mut App| open_about_window(cx));
    cx.on_action(quit);

    #[cfg(target_os = "macos")]
    cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());
}

pub(crate) fn app_menus(i18n: &I18n) -> Vec<Menu> {
    let mut app_items = vec![MenuItem::action(app_name_message(i18n, "menu-about"), About)];

    #[cfg(target_os = "macos")]
    {
        app_items.extend([
            MenuItem::separator(),
            MenuItem::os_submenu(i18n.t("menu-services"), SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(app_name_message(i18n, "menu-hide"), Hide),
            MenuItem::action(i18n.t("menu-hide-others"), HideOthers),
            MenuItem::action(i18n.t("menu-show-all"), ShowAll),
        ]);
    }

    app_items.extend([
        MenuItem::separator(),
        MenuItem::action(app_name_message(i18n, "menu-quit"), Quit),
    ]);

    vec![
        Menu {
            name: APP_TITLE.into(),
            items: app_items,
        },
        Menu {
            name: i18n.t("menu-file").into(),
            items: vec![MenuItem::action(i18n.t("menu-close-window"), CloseWindow)],
        },
        Menu {
            name: i18n.t("menu-window").into(),
            items: vec![
                MenuItem::action(i18n.t("menu-minimize"), Minimize),
                MenuItem::action(i18n.t("menu-zoom"), Zoom),
            ],
        },
    ]
}

fn app_name_message(i18n: &I18n, key: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("app_name", APP_TITLE);
    i18n.t_with_args(key, &args)
}

fn quit(_: &Quit, cx: &mut App) {
    event!(Level::INFO, "quit by action");
    cx.quit();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_names(items: Vec<MenuItem>) -> Vec<String> {
        items
            .into_iter()
            .map(|item| match item {
                MenuItem::Separator => "---".to_string(),
                MenuItem::Submenu(menu) => menu.name.to_string(),
                MenuItem::SystemMenu(menu) => menu.name.to_string(),
                MenuItem::Action { name, .. } => name.to_string(),
            })
            .collect()
    }

    #[test]
    fn builds_expected_top_level_menus() {
        let i18n = I18n::english_for_test();
        let menus = app_menus(&i18n);
        let names = menus
            .iter()
            .map(|menu| menu.name.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![APP_TITLE.to_string(), "File".into(), "Window".into()]
        );
    }

    #[test]
    fn builds_expected_app_menu_items() {
        let i18n = I18n::english_for_test();
        let mut menus = app_menus(&i18n);
        let app_menu = menus.remove(0);
        let item_names = item_names(app_menu.items);

        #[cfg(target_os = "macos")]
        assert_eq!(
            item_names,
            vec![
                format!("About {APP_TITLE}"),
                "---".into(),
                "Services".into(),
                "---".into(),
                format!("Hide {APP_TITLE}"),
                "Hide Others".into(),
                "Show All".into(),
                "---".into(),
                format!("Quit {APP_TITLE}"),
            ]
        );

        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            item_names,
            vec![
                format!("About {APP_TITLE}"),
                "---".into(),
                format!("Quit {APP_TITLE}"),
            ]
        );
    }
}
