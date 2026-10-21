//! A caption language's own name, for the `<track label>` the browser's
//! caption menu shows.
//!
//! The table is BCP 47's primary subtag to the name that language calls itself.
//! A regional tag reads through its primary subtag (`pt-BR` is `Português`). A
//! tag outside the table is labelled by its tag, which is what a reader who
//! asked for `gom` can still recognise.

/// The name the language calls itself, or the tag when it is not in the table.
#[must_use]
pub fn autonym(tag: &str) -> &str {
    let primary = subtag(tag);
    NAMES
        .iter()
        .find(|(code, _)| primary.eq_ignore_ascii_case(code))
        .map_or(tag, |(_, name)| name)
}

fn subtag(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

/// Every autonym this build knows, by primary subtag.
const NAMES: &[(&str, &str)] = &[
    ("af", "Afrikaans"),
    ("am", "አማርኛ"),
    ("ar", "العربية"),
    ("as", "অসমীয়া"),
    ("az", "Azərbaycanca"),
    ("be", "Беларуская"),
    ("bg", "Български"),
    ("bn", "বাংলা"),
    ("bo", "བོད་ཡིག"),
    ("bs", "Bosanski"),
    ("ca", "Català"),
    ("cs", "Čeština"),
    ("cy", "Cymraeg"),
    ("da", "Dansk"),
    ("de", "Deutsch"),
    ("dz", "རྫོང་ཁ"),
    ("el", "Ελληνικά"),
    ("en", "English"),
    ("eo", "Esperanto"),
    ("es", "Español"),
    ("et", "Eesti"),
    ("eu", "Euskara"),
    ("fa", "فارسی"),
    ("fi", "Suomi"),
    ("fil", "Filipino"),
    ("fo", "Føroyskt"),
    ("fr", "Français"),
    ("ga", "Gaeilge"),
    ("gd", "Gàidhlig"),
    ("gl", "Galego"),
    ("gu", "ગુજરાતી"),
    ("ha", "Hausa"),
    ("he", "עברית"),
    ("hi", "हिन्दी"),
    ("hr", "Hrvatski"),
    ("hu", "Magyar"),
    ("hy", "Հայերեն"),
    ("id", "Bahasa Indonesia"),
    ("is", "Íslenska"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("ka", "ქართული"),
    ("kk", "Қазақ тілі"),
    ("km", "ខ្មែរ"),
    ("kn", "ಕನ್ನಡ"),
    ("ko", "한국어"),
    ("ku", "Kurdî"),
    ("ky", "Кыргызча"),
    ("lo", "ລາວ"),
    ("lt", "Lietuvių"),
    ("lv", "Latviešu"),
    ("mk", "Македонски"),
    ("ml", "മലയാളം"),
    ("mn", "Монгол"),
    ("mr", "मराठी"),
    ("ms", "Bahasa Melayu"),
    ("my", "မြန်မာ"),
    ("nb", "Norsk bokmål"),
    ("ne", "नेपाली"),
    ("nl", "Nederlands"),
    ("nn", "Nynorsk"),
    ("no", "Norsk"),
    ("or", "ଓଡ଼ିଆ"),
    ("pa", "ਪੰਜਾਬੀ"),
    ("pl", "Polski"),
    ("ps", "پښتو"),
    ("pt", "Português"),
    ("ro", "Română"),
    ("ru", "Русский"),
    ("sd", "سنڌي"),
    ("si", "සිංහල"),
    ("sk", "Slovenčina"),
    ("sl", "Slovenščina"),
    ("so", "Soomaali"),
    ("sq", "Shqip"),
    ("sr", "Српски"),
    ("sv", "Svenska"),
    ("sw", "Kiswahili"),
    ("ta", "தமிழ்"),
    ("te", "తెలుగు"),
    ("th", "ไทย"),
    ("ti", "ትግርኛ"),
    ("tl", "Tagalog"),
    ("tr", "Türkçe"),
    ("tt", "Татарча"),
    ("ug", "ئۇيغۇرچە"),
    ("uk", "Українська"),
    ("ur", "اردو"),
    ("uz", "Oʻzbekcha"),
    ("vi", "Tiếng Việt"),
    ("yi", "ייִדיש"),
    ("yo", "Yorùbá"),
    ("zh", "中文"),
    ("zu", "isiZulu"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_language_names_itself() {
        assert_eq!(autonym("en"), "English");
        assert_eq!(autonym("fr"), "Français");
        assert_eq!(autonym("hi"), "हिन्दी");
    }

    #[test]
    fn a_region_reads_as_its_language() {
        assert_eq!(autonym("pt-BR"), "Português");
        assert_eq!(autonym("EN_gb"), "English");
    }

    #[test]
    fn an_unknown_tag_is_its_own_label() {
        assert_eq!(autonym("gom"), "gom");
    }
}
