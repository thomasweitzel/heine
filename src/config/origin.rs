//! Validated canonical site origins.

use url::Url;

use crate::output::OutputPath;

use super::BasePath;

/// An absolute HTTP(S) origin without a deployment path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SiteOrigin(Url);

impl SiteOrigin {
    fn canonical_url(&self, path: &str) -> String {
        format!("{}{}", self.0.as_str().trim_end_matches('/'), path)
    }
}

/// Canonical URL construction for one configured deployment.
///
/// Keeping the origin and deployment path together prevents consumers from
/// accidentally treating an origin as the site root when the site lives below
/// a subpath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalUrls {
    origin: SiteOrigin,
    base_path: BasePath,
}

impl CanonicalUrls {
    pub(crate) fn new(origin: SiteOrigin, base_path: BasePath) -> Self {
        Self { origin, base_path }
    }

    pub(crate) fn site_url(&self) -> String {
        self.origin.canonical_url(self.base_path.as_str())
    }

    pub(crate) fn output_url(&self, path: &OutputPath) -> String {
        self.origin
            .canonical_url(&self.base_path.join(&path.url_path()))
    }
}

impl TryFrom<String> for SiteOrigin {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let origin = Url::parse(&value)
            .map_err(|error| format!("site.origin must be an absolute HTTP(S) origin: {error}"))?;
        if !matches!(origin.scheme(), "http" | "https") {
            return Err("site.origin must use the http or https scheme".into());
        }
        if origin.host_str().is_none() {
            return Err("site.origin must include a host".into());
        }
        if !origin.username().is_empty() || origin.password().is_some() {
            return Err("site.origin must not include user credentials".into());
        }
        if origin.path() != "/" || origin.query().is_some() || origin.fragment().is_some() {
            return Err("site.origin must contain only scheme, host, and an optional port; put a deployment path in site.base_path".into());
        }
        Ok(Self(origin))
    }
}

#[cfg(test)]
mod tests {
    use super::SiteOrigin;

    #[test]
    fn accepts_only_a_scheme_host_and_optional_port() {
        let origin =
            SiteOrigin::try_from("https://example.org:8443".to_owned()).expect("valid origin");
        assert_eq!(
            origin.canonical_url("/guide/index.html"),
            "https://example.org:8443/guide/index.html"
        );

        for value in [
            "ftp://example.org",
            "https://example.org/guide/",
            "https://user@example.org",
            "https://example.org/?query=value",
            "https://example.org/#fragment",
        ] {
            assert!(SiteOrigin::try_from(value.to_owned()).is_err(), "{value}");
        }
    }

    #[test]
    fn joins_the_origin_with_the_complete_site_root_and_output_path() {
        let urls = super::CanonicalUrls::new(
            SiteOrigin::try_from("https://example.org".to_owned()).expect("valid origin"),
            "/guide/".try_into().expect("valid base path"),
        );
        let path = crate::output::OutputPath::try_new("blog/index.html".into())
            .expect("valid output path");

        assert_eq!(urls.site_url(), "https://example.org/guide/");
        assert_eq!(
            urls.output_url(&path),
            "https://example.org/guide/blog/index.html"
        );
    }
}
