//! Checked HTML class lists used by Markdown-owned output elements.

use std::collections::BTreeSet;

/// One normalized, non-empty list of distinct HTML class tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HtmlClassList(String);

impl HtmlClassList {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        let tokens = value.split_ascii_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            return Err("class must not be empty");
        }
        let mut seen = BTreeSet::new();
        if tokens.iter().any(|token| !seen.insert(*token)) {
            return Err("class must not repeat a token");
        }
        Ok(Self(tokens.join(" ")))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlClassList;

    #[test]
    fn normalizes_ascii_whitespace_and_rejects_ambiguous_lists() {
        assert_eq!(
            HtmlClassList::parse(" diagram\talign-center ")
                .expect("distinct tokens")
                .as_str(),
            "diagram align-center"
        );
        assert_eq!(HtmlClassList::parse("  "), Err("class must not be empty"));
        assert_eq!(
            HtmlClassList::parse("diagram diagram"),
            Err("class must not repeat a token")
        );
    }
}
