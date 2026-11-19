//! How a typed text is compared with a stored name.
//!
//! A search form is typed by a person, so both sides are folded before they
//! are compared: lower case, and the accent taken off a Latin letter, so that
//! `gonzalez` finds `González` and `LJUBOJEVIC` finds `Ljubojević`. The fold is
//! applied to each name once, when the query is compiled, never per game.

/// How a typed text must sit inside a stored name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextMatch {
    /// The name starts with the text.
    #[default]
    Prefix,
    /// The text appears anywhere in the name.
    Contains,
    /// The name is the text, once both are folded.
    Exact,
}

/// A typed text and how it must match, folded once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Needle {
    folded: String,
    how: TextMatch,
}

impl Needle {
    /// The needle for `text`, or `None` when the text is blank: a blank field
    /// of a search form is no criterion, not a criterion nothing satisfies.
    pub(crate) fn new(text: &str, how: TextMatch) -> Option<Needle> {
        let mut folded = String::with_capacity(text.len());
        fold_into(text.trim(), &mut folded);
        (!folded.is_empty()).then_some(Needle { folded, how })
    }

    /// Whether `name`, already folded with [`fold_into`], satisfies the needle.
    pub(crate) fn matches_folded(&self, name: &str) -> bool {
        match self.how {
            TextMatch::Prefix => name.starts_with(&self.folded),
            TextMatch::Contains => name.contains(&self.folded),
            TextMatch::Exact => name == self.folded,
        }
    }
}

/// Appends `text` to `out`, lower-cased and with Latin accents removed.
pub(crate) fn fold_into(text: &str, out: &mut String) {
    if text.is_ascii() {
        out.extend(text.bytes().map(|b| char::from(b.to_ascii_lowercase())));
        return;
    }
    for c in text.chars() {
        match base_letter(c) {
            Some(base) => out.push_str(base),
            None => out.extend(c.to_lowercase()),
        }
    }
}

/// The plain letters of an accented Latin letter, lower case; `None` for
/// anything else, which is then only lower-cased.
fn base_letter(c: char) -> Option<&'static str> {
    Some(match c {
        'À'..='Å' | 'à'..='å' | 'Ā' | 'ā' | 'Ă' | 'ă' | 'Ą' | 'ą' => "a",
        'Æ' | 'æ' => "ae",
        'Ç' | 'ç' | 'Ć' | 'ć' | 'Ĉ' | 'ĉ' | 'Ċ' | 'ċ' | 'Č' | 'č' => "c",
        'Ď' | 'ď' | 'Đ' | 'đ' | 'Ð' | 'ð' => "d",
        'È'..='Ë' | 'è'..='ë' | 'Ē' | 'ē' | 'Ĕ' | 'ĕ' | 'Ė' | 'ė' | 'Ę' | 'ę' | 'Ě' | 'ě' => "e",
        'Ĝ' | 'ĝ' | 'Ğ' | 'ğ' | 'Ġ' | 'ġ' | 'Ģ' | 'ģ' => "g",
        'Ĥ' | 'ĥ' | 'Ħ' | 'ħ' => "h",
        'Ì'..='Ï' | 'ì'..='ï' | 'Ĩ' | 'ĩ' | 'Ī' | 'ī' | 'Ĭ' | 'ĭ' | 'Į' | 'į' | 'İ' | 'ı' => "i",
        'Ĵ' | 'ĵ' => "j",
        'Ķ' | 'ķ' => "k",
        'Ĺ' | 'ĺ' | 'Ļ' | 'ļ' | 'Ľ' | 'ľ' | 'Ŀ' | 'ŀ' | 'Ł' | 'ł' => "l",
        'Ñ' | 'ñ' | 'Ń' | 'ń' | 'Ņ' | 'ņ' | 'Ň' | 'ň' => "n",
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' | 'Ō' | 'ō' | 'Ŏ' | 'ŏ' | 'Ő' | 'ő' => "o",
        'Œ' | 'œ' => "oe",
        'Ŕ' | 'ŕ' | 'Ŗ' | 'ŗ' | 'Ř' | 'ř' => "r",
        'Ś' | 'ś' | 'Ŝ' | 'ŝ' | 'Ş' | 'ş' | 'Š' | 'š' => "s",
        'ß' => "ss",
        'Ţ' | 'ţ' | 'Ť' | 'ť' | 'Ŧ' | 'ŧ' => "t",
        'Ù'..='Ü' | 'ù'..='ü' | 'Ũ' | 'ũ' | 'Ū' | 'ū' | 'Ŭ' | 'ŭ' | 'Ů' | 'ů' | 'Ű' | 'ű' | 'Ų' | 'ų' => {
            "u"
        }
        'Ŵ' | 'ŵ' => "w",
        'Ý' | 'ý' | 'ÿ' | 'Ŷ' | 'ŷ' | 'Ÿ' => "y",
        'Ź' | 'ź' | 'Ż' | 'ż' | 'Ž' | 'ž' => "z",
        'Þ' | 'þ' => "th",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folded(text: &str) -> String {
        let mut out = String::new();
        fold_into(text, &mut out);
        out
    }

    #[test]
    fn case_and_accents_fold_away() {
        assert_eq!(folded("González"), "gonzalez");
        assert_eq!(folded("Ljubojević"), "ljubojevic");
        assert_eq!(folded("Øystein Dannevig"), "oystein dannevig");
        assert_eq!(folded("Großmeister"), "grossmeister");
    }

    #[test]
    fn a_blank_text_is_no_criterion() {
        assert!(Needle::new("   ", TextMatch::Prefix).is_none());
    }

    #[test]
    fn each_mode_places_the_text_its_own_way() {
        let name = folded("Kasparov");
        assert!(Needle::new("KASP", TextMatch::Prefix).unwrap().matches_folded(&name));
        assert!(!Needle::new("parov", TextMatch::Prefix).unwrap().matches_folded(&name));
        assert!(Needle::new("parov", TextMatch::Contains).unwrap().matches_folded(&name));
        assert!(!Needle::new("Kasp", TextMatch::Exact).unwrap().matches_folded(&name));
        assert!(Needle::new("kasparov", TextMatch::Exact).unwrap().matches_folded(&name));
    }
}
