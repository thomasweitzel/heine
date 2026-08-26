//! Validated deployment-path configuration.
//!
//! A base path is both interpolated into generated URLs and used to route
//! development-server requests. Keeping its grammar here gives those separate
//! consumers one trusted, canonical configuration value.

/// A URL path prefix under which the generated site is deployed.
///
/// It is deliberately distinct from a full URL: the latter would carry an
/// origin and is only needed by features such as feeds or canonical links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BasePath(String);

impl BasePath {
    pub(crate) fn root() -> Self {
        Self("/".into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn join(&self, path: &crate::output::UrlPath) -> String {
        format!("{}{}", self.0, path.as_str())
    }
}

impl TryFrom<String> for BasePath {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.starts_with('/') {
            return Err("site.base_path must start with '/'; for example, '/docs/'".into());
        }
        if !value.ends_with('/') {
            return Err("site.base_path must end with '/'; for example, '/docs/'".into());
        }
        if !value.is_ascii() {
            return Err(
                "site.base_path must use ASCII URL-path characters; percent-encode non-ASCII characters"
                    .into(),
            );
        }
        if value != "/"
            && value
                .trim_matches('/')
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err("site.base_path must not contain empty, '.' or '..' path segments".into());
        }
        if value != "/"
            && value
                .trim_matches('/')
                .split('/')
                .any(|segment| !valid_url_segment(segment))
        {
            return Err(
                "site.base_path must use URL-path characters and uppercase percent escapes; percent-encode spaces and reserved characters"
                    .into(),
            );
        }
        Ok(Self(value))
    }
}

impl TryFrom<&str> for BasePath {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.to_owned().try_into()
    }
}

/// Accept URL path segments in their canonical, already-encoded form. Keeping
/// this representation canonical makes it safe to use both as a URL prefix
/// and as the injected reload-client URL without a second escaping boundary.
pub(crate) fn valid_url_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let Some(encoded) = bytes.get(index + 1..=index + 2) else {
                    return false;
                };
                if !encoded.iter().all(u8::is_ascii_hexdigit)
                    || encoded.iter().any(u8::is_ascii_lowercase)
                    || encoded == b"2E"
                    || encoded == b"2F"
                    || encoded == b"5C"
                    || encoded == b"00"
                {
                    return false;
                }
                index += 3;
            }
            byte if byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'-' | b'.'
                        | b'_'
                        | b'~'
                        | b'!'
                        | b'$'
                        | b'&'
                        | b'\''
                        | b'('
                        | b')'
                        | b'*'
                        | b'+'
                        | b','
                        | b';'
                        | b'='
                        | b':'
                        | b'@'
                ) =>
            {
                index += 1;
            }
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_percent_encoded_segments_only() {
        assert!(BasePath::try_from(String::from("/guide%20notes/")).is_ok());
        assert!(BasePath::try_from(String::from("/guide notes/")).is_err());
        assert!(BasePath::try_from(String::from("/guide%2fnotes/")).is_err());
        assert!(BasePath::try_from(String::from("/%2E/")).is_err());
        assert!(BasePath::try_from(String::from("/guide\"/")).is_err());
    }
}
