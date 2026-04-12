pub use fluent_bundle::FluentArgs;
use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

const EN_US_APP_ABOUT: &str = include_str!("../locales/en-US/app-about.ftl");
const EN_US_APP_HOME: &str = include_str!("../locales/en-US/app-home.ftl");
const EN_US_APP_MENU: &str = include_str!("../locales/en-US/app-menu.ftl");
const EN_US_PROVIDER_SQLITE: &str = include_str!("../locales/en-US/provider-sqlite.ftl");

const ZH_CN_APP_ABOUT: &str = include_str!("../locales/zh-CN/app-about.ftl");
const ZH_CN_APP_HOME: &str = include_str!("../locales/zh-CN/app-home.ftl");
const ZH_CN_APP_MENU: &str = include_str!("../locales/zh-CN/app-menu.ftl");
const ZH_CN_PROVIDER_SQLITE: &str = include_str!("../locales/zh-CN/provider-sqlite.ftl");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacOsBundleLocalization {
    pub bundle_locale_tag: &'static str,
    pub lproj_dir: &'static str,
}

const MACOS_BUNDLE_LOCALIZATIONS: [MacOsBundleLocalization; 2] = [
    MacOsBundleLocalization {
        bundle_locale_tag: "en-US",
        lproj_dir: "en-US.lproj",
    },
    MacOsBundleLocalization {
        bundle_locale_tag: "zh-Hans",
        lproj_dir: "zh-Hans.lproj",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Locale {
    EnUs,
    ZhCn,
}

pub struct Translator {
    locale: Locale,
}

impl Translator {
    pub fn detect_system() -> Self {
        Self::new(detect_locale())
    }

    pub fn for_locale_tag(locale: &str) -> Self {
        let locale = match normalize_locale(locale).filter(|id| id.language.as_str() == "zh") {
            Some(_) => Locale::ZhCn,
            None => Locale::EnUs,
        };

        Self::new(locale)
    }

    pub fn t(&self, key: &str) -> String {
        self.translate(key, None)
    }

    pub fn t_with_args(&self, key: &str, args: &FluentArgs<'_>) -> String {
        self.translate(key, Some(args))
    }

    pub fn locale_tag(&self) -> &'static str {
        match self.locale {
            Locale::EnUs => "en-US",
            Locale::ZhCn => "zh-CN",
        }
    }

    pub fn english_for_test() -> Self {
        Self::new(Locale::EnUs)
    }

    fn new(locale: Locale) -> Self {
        Self { locale }
    }

    fn translate(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        let bundle = self.bundle();
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

    fn bundle(&self) -> FluentBundle<FluentResource> {
        match self.locale {
            Locale::EnUs => build_bundle(
                "en-US",
                &[
                    EN_US_APP_MENU,
                    EN_US_APP_HOME,
                    EN_US_APP_ABOUT,
                    EN_US_PROVIDER_SQLITE,
                ],
            ),
            Locale::ZhCn => build_bundle(
                "zh-CN",
                &[
                    ZH_CN_APP_MENU,
                    ZH_CN_APP_HOME,
                    ZH_CN_APP_ABOUT,
                    ZH_CN_PROVIDER_SQLITE,
                ],
            ),
        }
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

fn build_bundle(lang: &str, sources: &[&str]) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = lang.parse().expect("valid language id");
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle.set_use_isolating(false);

    for source in sources {
        let resource =
            FluentResource::try_new((*source).to_string()).expect("valid fluent resource");
        bundle
            .add_resource(resource)
            .expect("resource can be added");
    }

    bundle
}

pub fn macos_bundle_localizations() -> &'static [MacOsBundleLocalization] {
    &MACOS_BUNDLE_LOCALIZATIONS
}

#[cfg(test)]
mod tests {
    use super::{Locale, Translator, locale_from_candidates, macos_bundle_localizations};

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
            locale_from_candidates(None, None, None, Some("zh_CN:en_US"), Some("en_US.UTF-8")),
            Locale::ZhCn
        );
    }

    #[test]
    fn explicit_message_locale_overrides_system_locale() {
        assert_eq!(
            locale_from_candidates(Some("en_US.UTF-8"), None, Some("zh_CN.UTF-8"), None, None),
            Locale::ZhCn
        );
    }

    #[test]
    fn system_locale_overrides_lang_fallback() {
        assert_eq!(
            locale_from_candidates(Some("zh_CN.UTF-8"), None, None, None, Some("en_US.UTF-8")),
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
        let i18n = Translator::english_for_test();

        assert_eq!(i18n.t("missing-key"), "missing-key");
    }

    #[test]
    fn chinese_bundle_loads_known_message() {
        let i18n = Translator::for_locale_tag("zh-CN");

        assert_eq!(i18n.t("app-home-sidebar-title"), "连接");
    }

    #[test]
    fn provider_messages_load_from_shared_resources() {
        let i18n = Translator::for_locale_tag("zh-CN");

        assert_eq!(i18n.t("provider-sqlite-sidebar-group-tables"), "表");
    }

    #[test]
    fn macos_bundle_localizations_match_supported_app_locales() {
        assert_eq!(
            macos_bundle_localizations(),
            &[
                super::MacOsBundleLocalization {
                    bundle_locale_tag: "en-US",
                    lproj_dir: "en-US.lproj",
                },
                super::MacOsBundleLocalization {
                    bundle_locale_tag: "zh-Hans",
                    lproj_dir: "zh-Hans.lproj",
                },
            ]
        );
    }
}
