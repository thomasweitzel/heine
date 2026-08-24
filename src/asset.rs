//! Discovery and output handling for files copied from `content/`.
//!
//! `AssetCatalog` is independent of page rendering. It owns the information
//! needed to copy assets and to expose a deliberately small asset view to
//! templates.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use miette::Diagnostic;
use sha2::Digest;
use thiserror::Error;
use walkdir::DirEntry;

use crate::{config::IntegrityAlgorithm, source_path};

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not access asset {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("asset {path} is outside content root {root}")]
    OutsideContent { path: PathBuf, root: PathBuf },
    #[error("asset path {path} must use UTF-8 components and must not contain backslashes")]
    NonUtf8Path { path: PathBuf },
}

#[derive(Debug, Error)]
pub(crate) enum ResolveError {
    #[error(
        "asset id must be a non-empty path relative to content/; it must not start with '/', contain '.', '..', or backslashes"
    )]
    InvalidId,
    #[error("asset {id:?} was not found")]
    NotFound { id: PathBuf },
}

/// Validated inventory of copied content files and their derived metadata.
#[derive(Debug, Clone)]
pub(crate) struct AssetCatalog {
    root: PathBuf,
    paths: Vec<String>,
    versions: BTreeMap<PathBuf, u64>,
    dimensions: BTreeMap<PathBuf, (u32, u32)>,
    latest_input: Option<SystemTime>,
}

impl AssetCatalog {
    /// Builds the published-asset inventory from files that are not page
    /// definitions or declared page-content inputs.
    pub(crate) fn discover(
        root: &Path,
        entries: &[DirEntry],
        content_inputs: &BTreeSet<PathBuf>,
    ) -> Result<Self, Error> {
        let mut paths = Vec::new();
        let mut versions = BTreeMap::new();
        let mut dimensions = BTreeMap::new();
        let mut latest_input: Option<SystemTime> = None;
        for entry in entries {
            if !entry.file_type().is_file()
                || entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "page")
                || entry.file_name() == std::ffi::OsStr::new("_directory.toml")
            {
                continue;
            }
            let relative = relative_path(root, entry.path())?;
            if !source_path::has_safe_components(&relative) {
                return Err(Error::NonUtf8Path {
                    path: entry.path().into(),
                });
            }
            if content_inputs.contains(&relative) {
                continue;
            }
            let modified = fs::metadata(entry.path())
                .and_then(|metadata| metadata.modified())
                .map_err(|source| Error::Io {
                    path: entry.path().into(),
                    source,
                })?;
            latest_input = Some(latest_input.map_or(modified, |latest| latest.max(modified)));
            process_versioned(&relative, &mut versions);
            if is_image(&relative)
                && let Ok(reader) = image::ImageReader::open(entry.path())
                && let Ok(size) = reader.into_dimensions()
            {
                dimensions.insert(relative.clone(), size);
            }
            paths.push(source_path::url(&relative));
        }
        paths.sort();
        Ok(Self {
            root: root.into(),
            paths,
            versions,
            dimensions,
            latest_input,
        })
    }

    pub(crate) fn paths(&self) -> &[String] {
        &self.paths
    }

    /// The most recently modified copied-asset input, used to invalidate
    /// template output that may depend on asset URLs or integrity metadata.
    pub(crate) fn latest_input(&self) -> Option<SystemTime> {
        self.latest_input
    }

    pub(crate) fn integrity_values(
        &self,
        algorithm: Option<IntegrityAlgorithm>,
    ) -> Result<BTreeMap<PathBuf, String>, Error> {
        let Some(algorithm) = algorithm else {
            return Ok(BTreeMap::new());
        };
        self.paths
            .iter()
            .map(|asset| {
                let path = PathBuf::from(asset);
                let source_path = self.root.join(&path);
                let bytes = fs::read(&source_path).map_err(|source| Error::Io {
                    path: source_path,
                    source,
                })?;
                Ok((path, integrity_value(algorithm, &bytes)))
            })
            .collect()
    }

    pub(crate) fn source_path(&self, asset: &str) -> PathBuf {
        self.root.join(asset)
    }

    pub(crate) fn resolve_id(&self, id: &str) -> Result<PathBuf, ResolveError> {
        let requested = PathBuf::from(id);
        if !source_path::is_logical_id(&requested) {
            return Err(ResolveError::InvalidId);
        }
        self.resolve_path(requested)
    }

    pub(crate) fn resolve_relative(
        &self,
        page_dir: &Path,
        id: &str,
    ) -> Result<PathBuf, ResolveError> {
        let requested = PathBuf::from(id);
        if !source_path::is_logical_id(&requested) {
            return Err(ResolveError::InvalidId);
        }
        self.resolve_path(page_dir.join(requested))
    }

    fn resolve_path(&self, requested: PathBuf) -> Result<PathBuf, ResolveError> {
        let resolved = if self.contains(&requested) {
            requested.clone()
        } else if let Some(version) = self.versions.get(&requested) {
            versioned_path(&requested, *version)
        } else {
            return Err(ResolveError::NotFound { id: requested });
        };
        self.contains(&resolved)
            .then_some(resolved)
            .ok_or(ResolveError::NotFound { id: requested })
    }

    pub(crate) fn links(
        &self,
        base_path: String,
        integrities: BTreeMap<PathBuf, String>,
    ) -> AssetLinks {
        AssetLinks {
            catalog: Arc::new(self.clone()),
            base_path,
            integrities,
        }
    }

    fn contains(&self, path: &Path) -> bool {
        self.paths.binary_search(&source_path::url(path)).is_ok()
    }
}

/// Resolved asset URLs and metadata for one render operation.
#[derive(Debug, Clone)]
pub(crate) struct AssetLinks {
    catalog: Arc<AssetCatalog>,
    base_path: String,
    integrities: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AssetLink {
    path: PathBuf,
    url: String,
    integrity: Option<String>,
    dimensions: Option<(u32, u32)>,
}

impl AssetLinks {
    pub(crate) fn resolve(&self, id: &str) -> Result<AssetLink, ResolveError> {
        let path = self.catalog.resolve_id(id)?;
        Ok(AssetLink {
            url: format!("{}{}", self.base_path, source_path::url(&path)),
            integrity: self.integrities.get(&path).cloned(),
            dimensions: self.catalog.dimensions.get(&path).copied(),
            path,
        })
    }

    /// Resolves the legacy template-filter syntax. A leading `/` is relative
    /// to `content/`; all other paths are relative to the current page.
    pub(crate) fn resolve_template_path(
        &self,
        page_dir: &Path,
        id: &str,
    ) -> Result<(PathBuf, bool), ResolveError> {
        match id.strip_prefix('/') {
            Some(id) => self.catalog.resolve_id(id).map(|path| (path, true)),
            None => self
                .catalog
                .resolve_relative(page_dir, id)
                .map(|path| (path, false)),
        }
    }

    pub(crate) fn dimensions(&self, path: &Path) -> Option<(u32, u32)> {
        self.catalog.dimensions.get(path).copied()
    }
}

impl AssetLink {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn integrity(&self) -> Option<&str> {
        self.integrity.as_deref()
    }

    pub(crate) fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }
}

fn integrity_value(algorithm: IntegrityAlgorithm, bytes: &[u8]) -> String {
    let digest = match algorithm {
        IntegrityAlgorithm::Sha256 => sha2::Sha256::digest(bytes).to_vec(),
        IntegrityAlgorithm::Sha384 => sha2::Sha384::digest(bytes).to_vec(),
        IntegrityAlgorithm::Sha512 => sha2::Sha512::digest(bytes).to_vec(),
    };
    let prefix = match algorithm {
        IntegrityAlgorithm::Sha256 => "sha256",
        IntegrityAlgorithm::Sha384 => "sha384",
        IntegrityAlgorithm::Sha512 => "sha512",
    };
    format!("{prefix}-{}", STANDARD.encode(digest))
}

fn relative_path(root: &Path, path: &Path) -> Result<PathBuf, Error> {
    source_path::relative_to(root, path).ok_or_else(|| Error::OutsideContent {
        path: path.into(),
        root: root.into(),
    })
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpeg" | "jpg" | "webp" | "png" | "gif"
            )
        })
}

fn process_versioned(path: &Path, versions: &mut BTreeMap<PathBuf, u64>) {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return;
    };
    let Some((base, version)) = stem.rsplit_once(".v") else {
        return;
    };
    let Ok(version) = version.parse() else {
        return;
    };
    if version == 0 {
        return;
    }
    let mut key = path.with_file_name(base);
    if let Some(extension) = path.extension() {
        key.set_extension(extension);
    }
    versions
        .entry(key)
        .and_modify(|latest| *latest = (*latest).max(version))
        .or_insert(version);
}

pub(crate) fn versioned_path(path: &Path, version: u64) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let filename = if extension.is_empty() {
        format!("{stem}.v{version}")
    } else {
        format!("{stem}.v{version}.{extension}")
    };
    path.with_file_name(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_link_clones_share_the_catalog() {
        let links = AssetLinks {
            catalog: Arc::new(AssetCatalog {
                root: PathBuf::new(),
                paths: vec![],
                versions: BTreeMap::new(),
                dimensions: BTreeMap::new(),
                latest_input: None,
            }),
            base_path: "/".into(),
            integrities: BTreeMap::new(),
        };

        let copy = links.clone();
        assert!(Arc::ptr_eq(&links.catalog, &copy.catalog));
    }
}
