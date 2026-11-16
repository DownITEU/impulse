//! The content locales Wand serves and the closest one to what the app asks for.

pub const ENGLISH: &str = "en-US";

const SERVED: [&str; 14] =
    [ENGLISH, "zh-CN", "fr-FR", "de-DE", "es-ES", "it-IT", "pt-BR", "pl-PL", "tr-TR", "ja-JP", "ko-KR", "hi-IN", "id-ID", "th-TH"];

/// The serde default of copies stored before locales: they were fetched in English.
pub fn english() -> String {
    ENGLISH.to_owned()
}

/// The locale Wand serves closest to `locale`: the exact tag, then the same language, else English.
pub fn served(locale: &model::Locale) -> &'static str {
    let tag = locale.as_str();
    SERVED
        .into_iter()
        .find(|served| served.eq_ignore_ascii_case(tag))
        .or_else(|| SERVED.into_iter().find(|served| language(served).eq_ignore_ascii_case(language(tag))))
        .unwrap_or(ENGLISH)
}

fn language(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_closest_served_locale_wins() {
        let served = |tag| served(&model::Locale::new(tag));
        assert_eq!(served("fr"), "fr-FR");
        assert_eq!(served("fr-CA"), "fr-FR");
        assert_eq!(served("zh-Hans"), "zh-CN");
        assert_eq!(served("EN"), "en-US");
        assert_eq!(served("xx"), "en-US");
        assert_eq!(served("pt-PT"), "pt-BR");
        assert_eq!(served("PT-br"), "pt-BR");
        assert_eq!(served("de_AT"), "de-DE");
        assert_eq!(served(""), "en-US");
    }
}
