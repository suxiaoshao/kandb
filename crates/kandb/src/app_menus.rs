use crate::{APP_TITLE, views::about::open_about_window};
use gpui::{App, KeyBinding, Menu, MenuItem, SystemMenuType, actions};
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

pub(crate) fn app_menus() -> Vec<Menu> {
    let mut app_items = vec![MenuItem::action(format!("About {APP_TITLE}"), About)];

    #[cfg(target_os = "macos")]
    {
        app_items.extend([
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(format!("Hide {APP_TITLE}"), Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
        ]);
    }

    app_items.extend([
        MenuItem::separator(),
        MenuItem::action(format!("Quit {APP_TITLE}"), Quit),
    ]);

    vec![
        Menu {
            name: APP_TITLE.into(),
            items: app_items,
        },
        Menu {
            name: "File".into(),
            items: vec![MenuItem::action("Close Window", CloseWindow)],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
            ],
        },
    ]
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
        let menus = app_menus();
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
        let mut menus = app_menus();
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
