#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app_menus;
mod app_paths;
mod components;
mod config;
mod errors;
mod views;
mod workspace;

use config::LoadedAppConfig;
use errors::{KandbError, KandbResult};
use gpui::*;
use gpui_component::Root;
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use views::home::HomeView;
use workspace::{LoadedWorkspaceState, WorkspaceState, WorkspaceStore};

pub(crate) static APP_ID: &str = "top.sushao.kandb";
pub(crate) static APP_TITLE: &str = "KanDB";

fn init(cx: &mut App) {
    gpui_component::init(cx);
    app_menus::init(cx);
    cx.set_menus(app_menus::app_menus());
    cx.activate(true);
    views::init(cx);
}

fn get_logs_dir() -> KandbResult<PathBuf> {
    #[cfg(target_os = "macos")]
    let path = dirs_next::home_dir()
        .ok_or(KandbError::LogFileNotFound)
        .map(|dir| dir.join("Library/Logs").join(APP_ID));

    #[cfg(not(target_os = "macos"))]
    let path = dirs_next::data_local_dir()
        .ok_or(KandbError::LogFileNotFound)
        .map(|dir| dir.join(APP_ID).join("logs"));

    if let Ok(path) = &path
        && !path.exists()
    {
        create_dir_all(path).map_err(|_| KandbError::LogFileNotFound)?;
    }

    path
}

fn init_tracing() -> KandbResult<()> {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .with_writer(
                    std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(get_logs_dir()?.join("data.log"))
                        .map_err(|_| KandbError::LogFileNotFound)?,
                )
                .with_filter(LevelFilter::INFO),
        )
        .with(
            fmt::layer()
                .with_timer(fmt::time::LocalTime::rfc_3339())
                .event_format(fmt::format().pretty())
                .with_filter(LevelFilter::INFO),
        )
        .init();

    Ok(())
}

fn main() -> KandbResult<()> {
    init_tracing()?;
    event!(Level::INFO, "startup begin");

    let span = tracing::info_span!("kandb");
    let _enter = span.enter();
    let config = LoadedAppConfig::load()?;
    let workspace_state = LoadedWorkspaceState::load_or_create(&config.paths)?;
    event!(
        Level::INFO,
        config_file = %config.paths.config_file().display(),
        data_dir = %config.paths.data_dir().display(),
        state_dir = %config.paths.state_dir().display(),
        workspace_state_file = %config.paths.workspace_state_file().display(),
        connection_count = config.file.connections.len(),
        resolved_connection_count = config.resolved_connections.len(),
        "app config loaded"
    );

    let app = Application::new().with_assets(kandb_assets::Assets::default());
    event!(Level::INFO, "app created");

    app.run(move |cx: &mut App| {
        init(cx);
        cx.set_global(config.clone());
        let workspace = cx.new(|_| WorkspaceState::new(workspace_state.clone()));
        cx.set_global(WorkspaceStore(workspace.clone()));

        let default_window_bounds = WindowBounds::centered(size(px(1200.0), px(800.0)), cx);
        let initial_window_bounds = workspace
            .read(cx)
            .window_bounds()
            .unwrap_or(default_window_bounds);

        if let Err(err) = cx.open_window(
            WindowOptions {
                window_bounds: Some(initial_window_bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some(APP_TITLE.into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| HomeView::new(window, cx));
                let focus_handle = view.read(cx).focus_handle(cx).clone();
                window.activate_window();
                window.focus(&focus_handle);
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            event!(Level::ERROR, "open main window: {}", err);
        }

        event!(Level::INFO, "window opened");
    });

    Ok(())
}
