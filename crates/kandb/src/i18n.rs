use gpui::{App, Global};
pub(crate) use kandb_i18n::FluentArgs;
use kandb_i18n::Translator;

pub(crate) struct I18n {
    translator: Translator,
}

impl Global for I18n {}

pub(crate) fn init_i18n(cx: &mut App) {
    cx.set_global(I18n::new(Translator::detect_system()));
}

impl I18n {
    pub(crate) fn new(translator: Translator) -> Self {
        Self { translator }
    }

    pub(crate) fn t(&self, key: &str) -> String {
        self.translator.t(key)
    }

    pub(crate) fn t_with_args(&self, key: &str, args: &FluentArgs<'_>) -> String {
        self.translator.t_with_args(key, args)
    }

    pub(crate) fn locale_tag(&self) -> &'static str {
        self.translator.locale_tag()
    }

    #[cfg(test)]
    pub(crate) fn english_for_test() -> Self {
        Self::new(Translator::english_for_test())
    }
}

#[cfg(test)]
mod tests {
    use super::I18n;

    #[test]
    fn english_falls_back_to_key_when_missing() {
        let i18n = I18n::english_for_test();

        assert_eq!(i18n.t("missing-key"), "missing-key");
    }

    #[test]
    fn chinese_bundle_loads_known_message() {
        let i18n = I18n::new(kandb_i18n::Translator::for_locale_tag("zh-CN"));

        assert_eq!(i18n.t("app-home-sidebar-title"), "连接");
    }
}
