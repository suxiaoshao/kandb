use crate::{
    app_paths::AppPaths,
    errors::{KandbError, KandbResult},
    views::home::sidebar_model::{migrate_legacy_node_id, persisted_connection_node_id},
};
use gpui::{
    App, Bounds, Context, Entity, Global, Pixels, Task, Timer, WindowBounds, point, px, size,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::ErrorKind,
    ops::Deref,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{Level, event};

const WORKSPACE_STATE_VERSION: u32 = 1;
const DEFAULT_SIDEBAR_WIDTH: f32 = 280.0;
const MIN_SIDEBAR_WIDTH: f32 = 220.0;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone)]
pub(crate) struct WorkspaceStore(pub(crate) Entity<WorkspaceState>);

impl Global for WorkspaceStore {}

impl Deref for WorkspaceStore {
    type Target = Entity<WorkspaceState>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoadedWorkspaceState {
    pub(crate) path: PathBuf,
    pub(crate) state: PersistedWorkspaceState,
}

#[derive(Debug)]
pub(crate) struct WorkspaceState {
    path: PathBuf,
    persisted: PersistedWorkspaceState,
    save_task: Option<Task<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct PersistedWorkspaceState {
    #[serde(default = "default_workspace_state_version")]
    version: u32,
    #[serde(default = "default_sidebar_width")]
    sidebar_width: f32,
    #[serde(default)]
    selected_node_id: Option<String>,
    #[serde(default)]
    expanded_node_ids: BTreeSet<String>,
    #[serde(default)]
    home_window: Option<PersistedWindowBounds>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum PersistedWindowMode {
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) struct PersistedWindowBounds {
    mode: PersistedWindowMode,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Default for PersistedWorkspaceState {
    fn default() -> Self {
        Self {
            version: WORKSPACE_STATE_VERSION,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            selected_node_id: None,
            expanded_node_ids: BTreeSet::new(),
            home_window: None,
        }
    }
}

impl LoadedWorkspaceState {
    pub(crate) fn load_or_create(paths: &AppPaths) -> KandbResult<Self> {
        paths.ensure_dirs()?;
        let path = paths.workspace_state_file().to_path_buf();
        match fs::read_to_string(&path) {
            Ok(content) => match parse_workspace_state_file(&path, &content) {
                Ok(mut state) => {
                    state.version = WORKSPACE_STATE_VERSION;
                    Ok(Self { path, state })
                }
                Err(err) => {
                    event!(Level::ERROR, "parse workspace_state.toml failed: {}", err);
                    let state = PersistedWorkspaceState::default();
                    write_workspace_state_file(&path, &state)?;
                    Ok(Self { path, state })
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let state = PersistedWorkspaceState::default();
                write_workspace_state_file(&path, &state)?;
                Ok(Self { path, state })
            }
            Err(source) => Err(KandbError::ReadWorkspaceStateFile { path, source }),
        }
    }
}

impl WorkspaceState {
    pub(crate) fn new(loaded: LoadedWorkspaceState) -> Self {
        Self {
            path: loaded.path,
            persisted: loaded.state,
            save_task: None,
        }
    }

    pub(crate) fn sidebar_width(&self) -> Pixels {
        px(self.persisted.sidebar_width.max(MIN_SIDEBAR_WIDTH))
    }

    pub(crate) fn set_sidebar_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let width = width.max(px(MIN_SIDEBAR_WIDTH));
        let width_value = f32::from(width);
        if (self.persisted.sidebar_width - width_value).abs() < f32::EPSILON {
            return;
        }

        self.persisted.sidebar_width = width_value;
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn selected_node_id(&self) -> Option<&str> {
        self.persisted.selected_node_id.as_deref()
    }

    pub(crate) fn select_node(&mut self, node_id: impl Into<String>, cx: &mut Context<Self>) {
        let node_id = node_id.into();
        if self.persisted.selected_node_id.as_deref() == Some(node_id.as_str()) {
            return;
        }

        self.persisted.selected_node_id = Some(node_id);
        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn expanded_node_ids(&self) -> &BTreeSet<String> {
        &self.persisted.expanded_node_ids
    }

    pub(crate) fn set_node_expanded(
        &mut self,
        node_id: impl Into<String>,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        let node_id = node_id.into();
        let changed = if expanded {
            self.persisted.expanded_node_ids.insert(node_id)
        } else {
            self.persisted.expanded_node_ids.remove(&node_id)
        };

        if !changed {
            return;
        }

        self.schedule_save(cx);
        cx.notify();
    }

    pub(crate) fn reconcile_sidebar_state(
        &mut self,
        valid_node_ids: &BTreeSet<String>,
        settled_connection_node_ids: &BTreeSet<String>,
        default_selected_node_id: Option<&str>,
        default_expanded_node_ids: &BTreeSet<String>,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;

        let migrated_selected = self
            .persisted
            .selected_node_id
            .as_deref()
            .and_then(migrate_legacy_node_id);
        if migrated_selected.is_some() {
            self.persisted.selected_node_id = migrated_selected;
            changed = true;
        }

        let migrated_expanded = self
            .persisted
            .expanded_node_ids
            .iter()
            .map(|node_id| migrate_legacy_node_id(node_id).unwrap_or_else(|| node_id.clone()))
            .collect::<BTreeSet<_>>();
        if migrated_expanded != self.persisted.expanded_node_ids {
            self.persisted.expanded_node_ids = migrated_expanded;
            changed = true;
        }

        if self.persisted.expanded_node_ids.is_empty() && !default_expanded_node_ids.is_empty() {
            self.persisted.expanded_node_ids = default_expanded_node_ids.clone();
            changed = true;
        }

        let before_selected = self.persisted.selected_node_id.clone();
        let before_expanded = self.persisted.expanded_node_ids.clone();
        self.persisted
            .expanded_node_ids
            .retain(|node_id| {
                if valid_node_ids.contains(node_id) {
                    return true;
                }

                persisted_connection_node_id(node_id)
                    .is_some_and(|connection_id| !settled_connection_node_ids.contains(&connection_id))
            });

        if self
            .persisted
            .selected_node_id
            .as_ref()
            .is_some_and(|node_id| {
                if valid_node_ids.contains(node_id) {
                    return false;
                }

                persisted_connection_node_id(node_id)
                    .is_none_or(|connection_id| settled_connection_node_ids.contains(&connection_id))
            })
        {
            self.persisted.selected_node_id = default_selected_node_id.map(ToOwned::to_owned);
        }

        if self.persisted.selected_node_id.is_none()
            && let Some(default_selected_node_id) = default_selected_node_id
        {
            self.persisted.selected_node_id = Some(default_selected_node_id.to_owned());
        }

        if self.persisted.selected_node_id != before_selected
            || self.persisted.expanded_node_ids != before_expanded
            || changed
        {
            self.schedule_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn window_bounds(&self) -> Option<WindowBounds> {
        self.persisted.home_window.map(Into::into)
    }

    pub(crate) fn set_window_bounds(&mut self, bounds: WindowBounds, cx: &mut Context<Self>) {
        let bounds = PersistedWindowBounds::from(bounds);
        if self.persisted.home_window == Some(bounds) {
            return;
        }

        self.persisted.home_window = Some(bounds);
        self.schedule_save(cx);
    }

    pub(crate) fn save_now(&self) -> KandbResult<()> {
        write_workspace_state_file(&self.path, &self.persisted)
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        let path = self.path.clone();
        let snapshot = self.persisted.clone();
        self.save_task = Some(cx.spawn(async move |_, _cx| {
            Timer::after(SAVE_DEBOUNCE).await;
            if let Err(err) = write_workspace_state_file(&path, &snapshot) {
                event!(Level::ERROR, "save workspace_state.toml failed: {}", err);
            }
        }));
    }
}

impl From<WindowBounds> for PersistedWindowBounds {
    fn from(value: WindowBounds) -> Self {
        let mode = match value {
            WindowBounds::Windowed(_) => PersistedWindowMode::Windowed,
            WindowBounds::Maximized(_) => PersistedWindowMode::Maximized,
            WindowBounds::Fullscreen(_) => PersistedWindowMode::Fullscreen,
        };
        let bounds = value.get_bounds();

        Self {
            mode,
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }
}

impl From<PersistedWindowBounds> for WindowBounds {
    fn from(value: PersistedWindowBounds) -> Self {
        let bounds = Bounds::new(
            point(px(value.x), px(value.y)),
            size(px(value.width), px(value.height)),
        );
        match value.mode {
            PersistedWindowMode::Windowed => WindowBounds::Windowed(bounds),
            PersistedWindowMode::Maximized => WindowBounds::Maximized(bounds),
            PersistedWindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
        }
    }
}

pub(crate) fn save_now(cx: &App) {
    if let Err(err) = cx.global::<WorkspaceStore>().read(cx).save_now() {
        event!(
            Level::ERROR,
            "save workspace_state.toml on quit failed: {}",
            err
        );
    }
}

fn default_workspace_state_version() -> u32 {
    WORKSPACE_STATE_VERSION
}

fn default_sidebar_width() -> f32 {
    DEFAULT_SIDEBAR_WIDTH
}

fn parse_workspace_state_file(path: &Path, content: &str) -> KandbResult<PersistedWorkspaceState> {
    toml::from_str(content).map_err(|source| KandbError::ParseWorkspaceStateFile {
        path: path.to_path_buf(),
        message: source.to_string(),
    })
}

fn write_workspace_state_file(path: &Path, state: &PersistedWorkspaceState) -> KandbResult<()> {
    let content = toml::to_string_pretty(state).map_err(|source| {
        KandbError::SerializeWorkspaceStateFile {
            path: path.to_path_buf(),
            message: source.to_string(),
        }
    })?;

    fs::write(path, content).map_err(|source| KandbError::WriteWorkspaceStateFile {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{LoadedWorkspaceState, PersistedWindowBounds, PersistedWindowMode, WorkspaceState};
    use crate::app_paths::AppPaths;
    use gpui::{Bounds, WindowBounds, point, px, size};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn load_or_create_creates_default_workspace_state_file() {
        let tempdir = tempdir().expect("tempdir");
        let paths =
            AppPaths::from_roots(tempdir.path().join("config"), tempdir.path().join("data"));

        let loaded = LoadedWorkspaceState::load_or_create(&paths).expect("load workspace state");

        assert!(paths.workspace_state_file().exists());
        assert_eq!(loaded.state.sidebar_width, 280.0);
    }

    #[test]
    fn invalid_workspace_state_file_resets_to_default() {
        let tempdir = tempdir().expect("tempdir");
        let paths =
            AppPaths::from_roots(tempdir.path().join("config"), tempdir.path().join("data"));
        paths.ensure_dirs().expect("ensure dirs");
        std::fs::write(paths.workspace_state_file(), "not = [valid").expect("write invalid file");

        let loaded = LoadedWorkspaceState::load_or_create(&paths).expect("load workspace state");

        assert_eq!(loaded.state.sidebar_width, 280.0);
    }

    #[test]
    fn persisted_window_bounds_roundtrip_window_bounds() {
        let bounds = WindowBounds::Maximized(Bounds::new(
            point(px(10.0), px(20.0)),
            size(px(1200.0), px(800.0)),
        ));

        let persisted = PersistedWindowBounds::from(bounds);
        let restored = WindowBounds::from(persisted);

        assert_eq!(restored, bounds);
    }

    #[test]
    fn persisted_window_bounds_preserve_mode() {
        let persisted = PersistedWindowBounds {
            mode: PersistedWindowMode::Fullscreen,
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };

        let restored = WindowBounds::from(persisted);

        assert!(matches!(restored, WindowBounds::Fullscreen(_)));
    }

    #[test]
    fn workspace_state_exposes_minimum_sidebar_width() {
        let state = WorkspaceState::new(LoadedWorkspaceState {
            path: PathBuf::from("/tmp/workspace_state.toml"),
            state: super::PersistedWorkspaceState {
                sidebar_width: 120.0,
                ..Default::default()
            },
        });

        assert_eq!(state.sidebar_width(), px(220.0));
    }
}
