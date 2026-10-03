//! Interface language as the Rust side needs it: only the tray menu is drawn
//! outside the web interface.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Fr,
}

pub struct TrayLabels {
    pub open: &'static str,
    pub quit: &'static str,
}

impl Locale {
    /// Reads a language tag such as `fr-FR`, `fr_CA` or `fr_FR.UTF-8`.
    /// Anything that is not French falls back to English.
    pub fn from_tag(tag: &str) -> Self {
        let language = tag.split(['-', '_', '.']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("fr") {
            Self::Fr
        } else {
            Self::En
        }
    }

    pub fn system() -> Self {
        sys_locale::get_locale().map_or(Self::En, |tag| Self::from_tag(&tag))
    }
}

pub fn tray_labels(locale: Locale) -> TrayLabels {
    match locale {
        Locale::En => TrayLabels {
            open: "Open GoXLR Hub",
            quit: "Quit",
        },
        Locale::Fr => TrayLabels {
            open: "Ouvrir GoXLR Hub",
            quit: "Quitter",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_tags_map_to_french() {
        for tag in ["fr", "fr-FR", "fr_CA", "FR-be", "fr_FR.UTF-8"] {
            assert_eq!(Locale::from_tag(tag), Locale::Fr, "{tag}");
        }
    }

    #[test]
    fn everything_else_falls_back_to_english() {
        for tag in [
            "en", "en-GB", "de-DE", "", "  ", "french", "f", "frr-FR", "C",
        ] {
            assert_eq!(Locale::from_tag(tag), Locale::En, "{tag:?}");
        }
    }

    #[test]
    fn tray_labels_are_translated() {
        assert_eq!(tray_labels(Locale::En).quit, "Quit");
        assert_eq!(tray_labels(Locale::Fr).quit, "Quitter");
        assert_ne!(tray_labels(Locale::En).open, tray_labels(Locale::Fr).open);
    }
}
