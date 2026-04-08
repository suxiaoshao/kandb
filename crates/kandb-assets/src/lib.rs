use anyhow::{Result, anyhow};
use gpui::{AssetSource, SharedString};
use gpui_component::IconNamed;
use std::{borrow::Cow, collections::BTreeSet};

macro_rules! define_icon_assets {
    ($( $variant:ident => $slug:literal ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum IconName {
            $( $variant, )+
        }

        impl IconNamed for IconName {
            fn path(self) -> SharedString {
                match self {
                    $( Self::$variant => concat!("icons/", $slug, ".svg").into(), )+
                }
            }
        }

        fn load_lucide_icon(path: &str) -> Option<&'static [u8]> {
            match path {
                $( concat!("icons/", $slug, ".svg") => Some(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../third_party/lucide/icons/",
                    $slug,
                    ".svg"
                ))), )+
                _ => None,
            }
        }

        fn list_lucide_icons(path: &str) -> Vec<SharedString> {
            let icons = [$( SharedString::from(concat!("icons/", $slug, ".svg")), )+];
            icons
                .into_iter()
                .filter(|icon| path.is_empty() || icon.as_ref().starts_with(path))
                .collect()
        }
    };
}

define_icon_assets!(
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    Columns3 => "columns-3",
    Database => "database",
    FolderClosed => "folder-closed",
    FolderOpen => "folder-open",
    Hash => "hash",
    HardDrive => "hard-drive",
    KeyRound => "key-round",
    ListTree => "list-tree",
    Plus => "plus",
    RefreshCw => "refresh-cw",
    Rows3 => "rows-3",
    Server => "server",
    SquareTerminal => "square-terminal",
    Table => "table",
    Trash2 => "trash-2",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProviderIconName {
    Sqlite,
}

impl IconNamed for ProviderIconName {
    fn path(self) -> SharedString {
        match self {
            Self::Sqlite => "icons/providers/sqlite.svg".into(),
        }
    }
}

#[derive(Default)]
struct KandbAssetSource;

impl AssetSource for KandbAssetSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if let Some(icon) = load_lucide_icon(path) {
            return Ok(Some(Cow::Borrowed(icon)));
        }

        match path {
            "icons/providers/sqlite.svg" => Ok(Some(Cow::Borrowed(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/icons/providers/sqlite.svg"
            ))))),
            _ => Err(anyhow!("could not find asset at path \"{path}\"")),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = list_lucide_icons(path);
        if path.is_empty() || "icons/providers/sqlite.svg".starts_with(path) {
            assets.push("icons/providers/sqlite.svg".into());
        }
        Ok(assets)
    }
}

pub struct Assets {
    kandb_assets: KandbAssetSource,
    component_assets: gpui_component_assets::Assets,
}

impl Default for Assets {
    fn default() -> Self {
        Self {
            kandb_assets: KandbAssetSource,
            component_assets: gpui_component_assets::Assets,
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        self.kandb_assets
            .load(path)
            .or_else(|_| self.component_assets.load(path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = BTreeSet::new();
        for asset in self.kandb_assets.list(path)? {
            names.insert(asset);
        }
        for asset in self.component_assets.list(path)? {
            names.insert(asset);
        }
        Ok(names.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, IconName, ProviderIconName};
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed;

    #[test]
    fn lucide_icon_paths_are_declared_explicitly() {
        assert_eq!(
            IconName::Database.path(),
            SharedString::from("icons/database.svg")
        );
        assert_eq!(
            IconName::ChevronRight.path(),
            SharedString::from("icons/chevron-right.svg")
        );
    }

    #[test]
    fn provider_icon_path_is_custom() {
        assert_eq!(
            ProviderIconName::Sqlite.path(),
            SharedString::from("icons/providers/sqlite.svg")
        );
    }

    #[test]
    fn assets_do_not_list_entire_lucide_catalog() {
        let assets = Assets::default();
        let list = assets.list("icons/").expect("list icons");

        assert!(list.contains(&SharedString::from("icons/database.svg")));
        assert!(!list.contains(&SharedString::from("icons/airplay.svg")));
    }

    #[test]
    fn sqlite_provider_icon_loads() {
        let assets = Assets::default();
        let icon = assets
            .load("icons/providers/sqlite.svg")
            .expect("load sqlite icon")
            .expect("sqlite icon exists");

        assert!(!icon.is_empty());
    }
}
