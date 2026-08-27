//! The fixed source layout of a Heine project.
//!
//! Build setup, source discovery, and development-server watching all use
//! this value so the project-owned input namespaces remain one policy.

use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "heine.toml";
const CONTENT_DIRECTORY: &str = "content";
const DATA_DIRECTORY: &str = "data";
const DEPLOY_DIRECTORY: &str = "deploy";
const TEMPLATES_DIRECTORY: &str = "templates";
const LOCALES_DIRECTORY: &str = "locales";
const LICENSES_DIRECTORY: &str = "LICENSES";
const INPUT_DIRECTORIES: [&str; 6] = [
    CONTENT_DIRECTORY,
    DATA_DIRECTORY,
    DEPLOY_DIRECTORY,
    TEMPLATES_DIRECTORY,
    LOCALES_DIRECTORY,
    LICENSES_DIRECTORY,
];

/// Whether a site-root-relative path starts inside one of Heine's source
/// domains. Configuration uses this same layout policy to keep output out of
/// every build input.
pub(crate) fn starts_with_input_root(path: &Path) -> bool {
    matches!(path.components().next(), Some(std::path::Component::Normal(name)) if name.to_str().is_some_and(|name| INPUT_DIRECTORIES.contains(&name)))
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectPaths {
    root: PathBuf,
    config: PathBuf,
    content: PathBuf,
    data: PathBuf,
    deploy: PathBuf,
    templates: PathBuf,
    locales: PathBuf,
    licenses: PathBuf,
}

impl ProjectPaths {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.into(),
            config: root.join(CONFIG_FILE),
            content: root.join(CONTENT_DIRECTORY),
            data: root.join(DATA_DIRECTORY),
            deploy: root.join(DEPLOY_DIRECTORY),
            templates: root.join(TEMPLATES_DIRECTORY),
            locales: root.join(LOCALES_DIRECTORY),
            licenses: root.join(LICENSES_DIRECTORY),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn config(&self) -> &Path {
        &self.config
    }

    pub(crate) fn content(&self) -> &Path {
        &self.content
    }

    pub(crate) fn data(&self) -> &Path {
        &self.data
    }

    pub(crate) fn deploy(&self) -> &Path {
        &self.deploy
    }

    pub(crate) fn templates(&self) -> &Path {
        &self.templates
    }

    pub(crate) fn locales(&self) -> &Path {
        &self.locales
    }
    pub(crate) fn licenses(&self) -> &Path {
        &self.licenses
    }

    pub(crate) fn is_site_root(&self) -> bool {
        self.config().is_file() && self.content().is_dir() && self.templates().is_dir()
    }

    /// Returns whether a path belongs to the inputs that can affect a build.
    pub(crate) fn is_build_input(&self, path: &Path) -> bool {
        path == self.config
            || [
                &self.content,
                &self.data,
                &self.deploy,
                &self.templates,
                &self.locales,
                &self.licenses,
            ]
            .iter()
            .any(|directory| path.starts_with(directory))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_build_inputs_without_including_output() {
        let paths = ProjectPaths::new(Path::new("/site"));

        assert!(paths.is_build_input(Path::new("/site/heine.toml")));
        assert!(paths.is_build_input(Path::new("/site/locales/en/site.ftl")));
        assert!(paths.is_build_input(Path::new("/site/content/post.md")));
        assert!(paths.is_build_input(Path::new("/site/data/site.toml")));
        assert!(paths.is_build_input(Path::new("/site/deploy/.htaccess")));
        assert!(paths.is_build_input(Path::new("/site/LICENSES/MIT.txt")));
        assert!(paths.is_build_input(Path::new("/site/templates/base.tera")));
        assert!(!paths.is_build_input(Path::new("/site/public/index.html")));
    }

    #[test]
    fn recognizes_every_reserved_input_root_for_output_validation() {
        for root in [
            "content",
            "data",
            "deploy",
            "templates",
            "locales",
            "LICENSES",
        ] {
            assert!(starts_with_input_root(
                Path::new(root).join("nested").as_path()
            ));
        }
        assert!(!starts_with_input_root(Path::new("public/nested")));
    }
}
