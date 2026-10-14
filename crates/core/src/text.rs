//! Rules for every piece of text a user or an agent can enter. Text is hostile: it is trimmed,
//! bounded and free of control characters before it reaches the model.

/// Longest title in characters.
pub const TITLE_MAX: usize = 200;
/// Longest description or comment in characters.
pub const BODY_MAX: usize = 20_000;
/// Longest short label (kind, reference, role, initials, tag, e-mail) in characters.
pub const LABEL_MAX: usize = 80;
/// Most tags on one task.
pub const TAGS_MAX: usize = 30;

/// Why a text was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// Nothing left after trimming.
    #[error("must not be empty")]
    Empty,
    /// More characters than allowed.
    #[error("must not be longer than {max} characters")]
    TooLong {
        /// The limit.
        max: usize,
    },
    /// More items than allowed.
    #[error("must not have more than {max} entries")]
    TooMany {
        /// The limit.
        max: usize,
    },
    /// A control or invisible formatting character (line breaks only allowed in long texts).
    #[error("must not contain control or invisible characters")]
    ControlChar,
}

/// Invisible characters that can make text read differently than it is stored, or carry
/// hidden instructions to an AI assistant ("Trojan Source", "ASCII smuggling"): the Unicode
/// format category (Cf) as of Unicode 16, line and paragraph separators, and the tag block.
///
/// Deliberately allowed, because real writing needs them: soft hyphen (U+00AD, from word
/// processors), zero-width non-joiner and joiner (U+200C/U+200D: Persian, Indic scripts, emoji
/// such as 👩‍🚒) and the left-to-right / right-to-left marks (U+200E/U+200F: Hebrew and Arabic
/// names). Bidirectional overrides and isolates stay forbidden.
pub fn is_hidden(c: char) -> bool {
    matches!(
        c,
        '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{180E}'
            | '\u{200B}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    )
}

fn single_line(text: &str, max: usize) -> Result<String, TextError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(TextError::Empty);
    }
    if trimmed.chars().any(|c| c.is_control() || is_hidden(c)) {
        return Err(TextError::ControlChar);
    }
    if trimmed.chars().count() > max {
        return Err(TextError::TooLong { max });
    }
    Ok(trimmed.to_owned())
}

/// A title: trimmed, 1 to [`TITLE_MAX`] characters, one line.
pub fn title(text: &str) -> Result<String, TextError> {
    single_line(text, TITLE_MAX)
}

/// A short label: trimmed, 1 to [`LABEL_MAX`] characters, one line.
pub fn label(text: &str) -> Result<String, TextError> {
    single_line(text, LABEL_MAX)
}

/// A long text (Markdown): up to [`BODY_MAX`] characters; line breaks and tabs allowed.
pub fn body(text: &str) -> Result<String, TextError> {
    if text
        .chars()
        .any(|c| (c.is_control() && !matches!(c, '\n' | '\r' | '\t')) || is_hidden(c))
    {
        return Err(TextError::ControlChar);
    }
    if text.chars().count() > BODY_MAX {
        return Err(TextError::TooLong { max: BODY_MAX });
    }
    Ok(text.to_owned())
}

/// Tags: each a [`label`], duplicates removed, sorted, at most [`TAGS_MAX`].
pub fn tags(input: &[String]) -> Result<Vec<String>, TextError> {
    let mut out = input
        .iter()
        .map(|tag| label(tag))
        .collect::<Result<Vec<_>, _>>()?;
    out.sort();
    out.dedup();
    if out.len() > TAGS_MAX {
        return Err(TextError::TooMany { max: TAGS_MAX });
    }
    Ok(out)
}

/// Whether a stored title is exactly what [`title`] would produce.
pub fn is_title(text: &str) -> bool {
    title(text).is_ok_and(|t| t == text)
}

/// Whether a stored label is exactly what [`label`] would produce.
pub fn is_label(text: &str) -> bool {
    label(text).is_ok_and(|t| t == text)
}

/// Whether a stored long text is valid.
pub fn is_body(text: &str) -> bool {
    body(text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed() {
        assert_eq!(title("  Lagebesprechung  ").unwrap(), "Lagebesprechung");
    }

    #[test]
    fn empty_title_is_rejected() {
        assert_eq!(title(""), Err(TextError::Empty));
        assert_eq!(title("   \t"), Err(TextError::Empty));
    }

    #[test]
    fn title_length_counts_characters_not_bytes() {
        assert!(title(&"ä".repeat(200)).is_ok());
        assert_eq!(
            title(&"a".repeat(201)),
            Err(TextError::TooLong { max: TITLE_MAX })
        );
    }

    #[test]
    fn control_characters_are_rejected_in_titles() {
        assert_eq!(title("a\u{7}b"), Err(TextError::ControlChar));
        assert_eq!(title("a\nb"), Err(TextError::ControlChar));
    }

    #[test]
    fn invisible_characters_are_rejected() {
        let smuggled = format!("Bericht{}", "\u{E0049}\u{E0067}\u{E006E}");
        for bad in [
            "a\u{202E}b",
            "a\u{200B}b",
            "\u{FEFF}a",
            "a\u{2066}b",
            "a\u{2028}b",
            "a\u{2029}b",
            "a\u{061C}b",
            "a\u{180E}b",
            "a\u{FFF9}b",
            smuggled.as_str(),
        ] {
            assert_eq!(title(bad), Err(TextError::ControlChar), "{bad:?}");
            assert_eq!(body(bad), Err(TextError::ControlChar), "{bad:?}");
        }
    }

    #[test]
    fn characters_real_writing_needs_are_allowed() {
        for good in [
            "Einsatz 👩\u{200D}🚒",
            "می\u{200C}خواهم",
            "\u{200F}שלום",
            "Ein\u{00AD}satz\u{00AD}leitung",
        ] {
            assert!(title(good).is_ok(), "{good:?}");
            assert!(body(good).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn body_keeps_line_breaks_and_tabs() {
        assert_eq!(body("a\nb\tc\r\n").unwrap(), "a\nb\tc\r\n");
        assert_eq!(body(""), Ok(String::new()));
        assert_eq!(body("a\u{0}"), Err(TextError::ControlChar));
        assert!(body(&"x".repeat(BODY_MAX + 1)).is_err());
    }

    #[test]
    fn label_is_short_and_not_empty() {
        assert_eq!(label(" SKA ").unwrap(), "SKA");
        assert_eq!(label(""), Err(TextError::Empty));
        assert!(label(&"x".repeat(LABEL_MAX + 1)).is_err());
    }

    #[test]
    fn tags_are_deduplicated_and_sorted() {
        let input = vec!["b".to_owned(), " a".to_owned(), "b ".to_owned()];
        assert_eq!(tags(&input).unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn too_many_tags_are_rejected() {
        let many: Vec<String> = (0..=TAGS_MAX).map(|i| format!("t{i}")).collect();
        assert_eq!(tags(&many), Err(TextError::TooMany { max: TAGS_MAX }));
    }

    #[test]
    fn checks_report_whether_stored_text_is_valid() {
        assert!(is_title("ok") && !is_title(" padded "));
        assert!(is_label("ok") && !is_label(""));
        assert!(is_body("a\nb") && !is_body("\u{1b}"));
    }
}
