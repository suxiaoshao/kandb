use std::collections::HashMap;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui::{App, Global};
use unic_langid::LanguageIdentifier;

const EN_US: &str = include_str!("../locales/en-US/main.ftl");
const ZH_CN: &str = include_str!("../locales/zh-CN/main.ftl");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Locale {
    EnUs,
    ZhCn,
}

pub(crate) struct I18n {
    locale: Locale,
    bundles: HashMap<Locale, FluentBundle<FluentResource>>,
}

impl Global for I18n {}

pub(crate) fn init_i18n(cx: &mut App) {
    cx.set_global(I18n::new(detect_locale()));
}

impl I18n {
    fn new(locale: Locale) -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Locale::EnUs, build_bundle("en-US", EN_US));
        bundles.insert(Locale::ZhCn, build_bundle("zh-CN", ZH_CN));

        Self { locale, bundles }
    }

    pub(crate) fn t(&self, key: &str) -> String {
        self.translate(key, None)
    }

    pub(crate) fn t_with_args(&self, key: &str, args: &FluentArgs<'_>) -> String {
        self.translate(key, Some(args))
    }

    fn translate(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        let Some(bundle) = self.bundle() else {
            return key.to_string();
        };
        let Some(message) = bundle.get_message(key) else {
            return key.to_string();
        };
        let Some(pattern) = message.value() else {
            return key.to_string();
        };

        let mut errors = vec![];
        let text = bundle.format_pattern(pattern, args, &mut errors);
        if errors.is_empty() {
            text.to_string()
        } else {
            key.to_string()
        }
    }

    fn bundle(&self) -> Option<&FluentBundle<FluentResource>> {
        self.bundles
            .get(&self.locale)
            .or_else(|| self.bundles.get(&Locale::EnUs))
    }

    #[cfg(test)]
    pub(crate) fn english_for_test() -> Self {
        Self::new(Locale::EnUs)
    }
}

fn detect_locale() -> Locale {
    locale_from_candidates(
        sys_locale::get_locale().as_deref(),
        read_env_locale("LC_ALL").as_deref(),
        read_env_locale("LC_MESSAGES").as_deref(),
        read_env_locale("LANGUAGE").as_deref(),
        read_env_locale("LANG").as_deref(),
    )
}

fn locale_from_candidates(
    sys_locale: Option<&str>,
    lc_all: Option<&str>,
    lc_messages: Option<&str>,
    language: Option<&str>,
    lang: Option<&str>,
) -> Locale {
    let locale = [
        lc_messages,
        language,
        sys_locale,
        lang,
        lc_all.filter(|value| !is_neutral_locale(value)),
    ]
        .into_iter()
        .flatten()
        .find_map(normalize_locale);

    match locale.filter(|id| id.language.as_str() == "zh") {
        Some(_) => Locale::ZhCn,
        None => Locale::EnUs,
    }
}

fn is_neutral_locale(value: &str) -> bool {
    value
        .split(':')
        .all(|candidate| matches!(candidate.trim(), "C" | "POSIX" | "C.UTF-8"))
}

fn read_env_locale(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_locale(value: &str) -> Option<LanguageIdentifier> {
    value.split(':').find_map(|candidate| {
        let normalized = candidate
            .split(['.', '@'])
            .next()
            .unwrap_or(candidate)
            .replace('_', "-");

        normalized.parse::<LanguageIdentifier>().ok()
    })
}

fn build_bundle(lang: &str, source: &str) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = lang.parse().expect("valid language id");
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle.set_use_isolating(false);
    let resource = FluentResource::try_new(source.to_string()).expect("valid fluent resource");
    bundle
        .add_resource(resource)
        .expect("resource can be added");
    bundle
}

#[cfg(test)]
mod tests {
    use super::{I18n, Locale, locale_from_candidates};

    #[test]
    fn chinese_locale_maps_to_zh_cn() {
        assert_eq!(
            locale_from_candidates(Some("zh_CN.UTF-8"), None, None, None, None),
            Locale::ZhCn
        );
    }

    #[test]
    fn english_is_default_when_candidates_are_missing() {
        assert_eq!(
            locale_from_candidates(None, None, None, None, None),
            Locale::EnUs
        );
    }

    #[test]
    fn invalid_earlier_locale_does_not_block_valid_later_locale() {
        assert_eq!(
            locale_from_candidates(None, Some("C"), None, None, Some("zh_CN.UTF-8")),
            Locale::ZhCn
        );
    }

    #[test]
    fn language_locale_list_uses_first_valid_entry() {
        assert_eq!(
            locale_from_candidates(None, None, None, Some("zh_CN:en_US"), None),
            Locale::ZhCn
        );
    }

    #[test]
    fn lc_messages_overrides_language_and_lang() {
        assert_eq!(
            locale_from_candidates(
                None,
                None,
                Some("zh_CN.UTF-8"),
                Some("en_US:zh_CN"),
                Some("en_US.UTF-8")
            ),
            Locale::ZhCn
        );
    }

    #[test]
    fn language_overrides_lang() {
        assert_eq!(
            locale_from_candidates(
                None,
                None,
                None,
                Some("zh_CN:en_US"),
                Some("en_US.UTF-8")
            ),
            Locale::ZhCn
        );
    }

    #[test]
    fn explicit_message_locale_overrides_system_locale() {
        assert_eq!(
            locale_from_candidates(
                Some("en_US.UTF-8"),
                None,
                Some("zh_CN.UTF-8"),
                None,
                None
            ),
            Locale::ZhCn
        );
    }

    #[test]
    fn system_locale_overrides_lang_fallback() {
        assert_eq!(
            locale_from_candidates(
                Some("zh_CN.UTF-8"),
                None,
                None,
                None,
                Some("en_US.UTF-8")
            ),
            Locale::ZhCn
        );
    }

    #[test]
    fn neutral_lc_all_does_not_override_other_candidates() {
        assert_eq!(
            locale_from_candidates(
                Some("zh_CN.UTF-8"),
                Some("C.UTF-8"),
                None,
                None,
                Some("en_US.UTF-8")
            ),
            Locale::ZhCn
        );
    }

    #[test]
    fn english_falls_back_to_key_when_missing() {
        let i18n = I18n::english_for_test();

        assert_eq!(i18n.t("missing-key"), "missing-key");
    }

    #[test]
    fn chinese_bundle_loads_known_message() {
        let i18n = I18n::new(Locale::ZhCn);

        assert_eq!(i18n.t("home-sidebar-title"), "连接");
    }
}
