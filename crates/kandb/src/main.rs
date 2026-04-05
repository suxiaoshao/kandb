#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod errors;
mod views;

use errors::{KandbError, KandbResult};
use gpui::*;
use gpui_component::Root;
use std::{fs::create_dir_all, path::PathBuf};
use tracing::{Level, event, level_filters::LevelFilter};
use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use views::home::HomeView;

static APP_ID: &str = "top.sushao.kandb";
static APP_TITLE: &str = "kanDB";

actions!(kandb, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    event!(Level::INFO, "quit by action");
    cx.quit();
}

fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.activate(true);
    cx.on_action(quit);

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

    let app = Application::new().with_assets(gpui_component_assets::Assets);
    event!(Level::INFO, "app created");

    app.run(|cx: &mut App| {
        init(cx);
        if let Err(err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(APP_TITLE.into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| HomeView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            event!(Level::ERROR, "open main window: {}", err);
        }

        event!(Level::INFO, "window opened");
    });

    Ok(())
}
