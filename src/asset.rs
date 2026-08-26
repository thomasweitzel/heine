//! Discovery and locale-aware resolution of copied content assets.

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

use crate::{
    config::{BasePath, IntegrityAlgorithm},
    locale::Locale,
    output::OutputPath,
    source_path::{self, LogicalPath},
};

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
    #[error("asset {asset_path} would be published to invalid output path {output}: {message}")]
    InvalidOutputPath {
        asset_path: PathBuf,
        output: PathBuf,
        message: String,
    },
    #[error("assets {first} and {second} would both be published as {output}")]
    OutputCollision {
        output: PathBuf,
        first: PathBuf,
        second: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum ResolveError {
    #[error(
        "asset id must be a non-empty path relative to the current locale content root; it must not start with '/', contain '.', '..', or backslashes"
    )]
    InvalidId,
    #[error("asset {id:?} was not found")]
    NotFound { id: PathBuf },
}

/// A locale source tree plus its optional shared-asset fallback.
pub(crate) struct AssetSources<'a> {
    pub(crate) root: &'a Path,
    pub(crate) entries: &'a [DirEntry],
    pub(crate) content_inputs: &'a BTreeSet<PathBuf>,
    pub(crate) output_prefix: &'a Path,
    pub(crate) shared: Option<SharedAssets<'a>>,
}

pub(crate) struct SharedAssets<'a> {
    pub(crate) root: &'a Path,
    pub(crate) entries: &'a [DirEntry],
}

#[derive(Debug, Clone)]
struct AssetEntry {
    source: PathBuf,
    dimensions: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAsset {
    id: AssetId,
    pub(crate) output: OutputPath,
}

impl ResolvedAsset {
    pub(crate) fn relative_url_path(&self, page_dir: &Path) -> crate::output::UrlPath {
        let path = self
            .id
            .as_path()
            .strip_prefix(page_dir)
            .unwrap_or(self.id.as_path());
        let path = LogicalPath::from_path(path)
            .expect("a relative path below validated logical source paths remains logical");
        crate::output::UrlPath::from_logical(&path)
    }
}

/// A validated asset lookup identity within one locale content tree.
///
/// Unlike an [`OutputPath`], this is never a destination: it is the logical
/// name accepted by `asset()` and Markdown `asset:` links.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AssetId(LogicalPath);

impl AssetId {
    fn from_path(path: &Path) -> Option<Self> {
        LogicalPath::from_path(path).map(Self)
    }

    fn parse(value: &str) -> Option<Self> {
        LogicalPath::parse(value).map(Self)
    }

    fn as_path(&self) -> &Path {
        self.0.as_path()
    }
}

/// Validated inventory of copied assets. Logical IDs resolve per locale;
/// output paths remain global for the output domain.
#[derive(Debug, Clone)]
pub(crate) struct AssetCatalog {
    entries: BTreeMap<OutputPath, AssetEntry>,
    paths: Vec<OutputPath>,
    locale_entries: BTreeMap<Locale, BTreeMap<AssetId, OutputPath>>,
    versions: BTreeMap<(Locale, AssetId), u64>,
    latest_input: Option<SystemTime>,
}

impl AssetCatalog {
    /// Single-root constructor retained for focused output-domain tests.
    #[cfg(test)]
    pub(crate) fn discover(
        root: &Path,
        entries: &[DirEntry],
        content_inputs: &BTreeSet<PathBuf>,
    ) -> Result<Self, Error> {
        let locale: Locale = "und".parse().expect("und is valid BCP-47");
        let sources = BTreeMap::from([(
            locale,
            AssetSources {
                root,
                entries,
                content_inputs,
                output_prefix: Path::new(""),
                shared: None,
            },
        )]);
        Self::discover_locales(&sources)
    }

    pub(crate) fn discover_locales(
        sources: &BTreeMap<Locale, AssetSources<'_>>,
    ) -> Result<Self, Error> {
        let mut catalog = Self {
            entries: BTreeMap::new(),
            paths: Vec::new(),
            locale_entries: BTreeMap::new(),
            versions: BTreeMap::new(),
            latest_input: None,
        };
        for (locale, source) in sources {
            catalog.add_locale(locale, source)?;
        }
        catalog.paths = catalog.entries.keys().cloned().collect();
        Ok(catalog)
    }

    fn add_locale(&mut self, locale: &Locale, sources: &AssetSources<'_>) -> Result<(), Error> {
        let mut ids = BTreeMap::new();
        if let Some(shared) = &sources.shared {
            for entry in shared.entries {
                if entry.file_type().is_file() {
                    let relative = relative_path(shared.root, entry.path())?;
                    let id = Path::new("assets").join(relative);
                    let Some(id) = AssetId::from_path(&id) else {
                        return Err(Error::NonUtf8Path {
                            path: entry.path().into(),
                        });
                    };
                    let output = id.as_path().into();
                    let output = OutputPath::try_new(output).map_err(|message| {
                        Error::InvalidOutputPath {
                            asset_path: entry.path().into(),
                            output: id.as_path().into(),
                            message,
                        }
                    })?;
                    self.add_entry(locale, &mut ids, id.clone(), output, entry.path())?;
                }
            }
        }
        for entry in sources.entries {
            if !entry.file_type().is_file()
                || entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "page")
                || entry.file_name() == std::ffi::OsStr::new("_directory.toml")
            {
                continue;
            }
            let relative = relative_path(sources.root, entry.path())?;
            if !source_path::has_safe_components(&relative) {
                return Err(Error::NonUtf8Path {
                    path: entry.path().into(),
                });
            }
            if sources.content_inputs.contains(&relative) {
                continue;
            }
            let Some(id) = AssetId::from_path(&relative) else {
                return Err(Error::NonUtf8Path {
                    path: entry.path().into(),
                });
            };
            let output_path = sources.output_prefix.join(id.as_path());
            let output = OutputPath::try_new(output_path.clone()).map_err(|message| {
                Error::InvalidOutputPath {
                    asset_path: entry.path().into(),
                    output: output_path,
                    message,
                }
            })?;
            self.add_entry(locale, &mut ids, id.clone(), output, entry.path())?;
        }
        self.locale_entries.insert(locale.clone(), ids);
        Ok(())
    }

    fn add_entry(
        &mut self,
        locale: &Locale,
        ids: &mut BTreeMap<AssetId, OutputPath>,
        id: AssetId,
        output: OutputPath,
        source: &Path,
    ) -> Result<(), Error> {
        if !source_path::has_safe_components(id.as_path())
            || !source_path::has_safe_components(output.as_path())
        {
            return Err(Error::NonUtf8Path {
                path: source.into(),
            });
        }
        let modified = fs::metadata(source)
            .and_then(|metadata| metadata.modified())
            .map_err(|source_error| Error::Io {
                path: source.into(),
                source: source_error,
            })?;
        self.latest_input = Some(
            self.latest_input
                .map_or(modified, |latest| latest.max(modified)),
        );
        process_versioned(locale, &id, &mut self.versions);
        let dimensions = is_image(id.as_path())
            .then(|| {
                image::ImageReader::open(source)
                    .ok()?
                    .into_dimensions()
                    .ok()
            })
            .flatten();
        if let Some(existing) = self.entries.get(&output) {
            if existing.source != source {
                return Err(Error::OutputCollision {
                    output: output.as_path().into(),
                    first: existing.source.clone(),
                    second: source.into(),
                });
            }
        } else {
            self.entries.insert(
                output.clone(),
                AssetEntry {
                    source: source.into(),
                    dimensions,
                },
            );
        }
        ids.insert(id, output);
        Ok(())
    }

    pub(crate) fn paths(&self) -> &[OutputPath] {
        &self.paths
    }

    pub(crate) fn latest_input(&self) -> Option<SystemTime> {
        self.latest_input
    }

    pub(crate) fn integrity_values(
        &self,
        algorithm: Option<IntegrityAlgorithm>,
    ) -> Result<BTreeMap<OutputPath, String>, Error> {
        let Some(algorithm) = algorithm else {
            return Ok(BTreeMap::new());
        };
        self.entries
            .iter()
            .map(|(output, entry)| {
                let bytes = fs::read(&entry.source).map_err(|source| Error::Io {
                    path: entry.source.clone(),
                    source,
                })?;
                Ok((output.clone(), integrity_value(algorithm, &bytes)))
            })
            .collect()
    }

    pub(crate) fn source_path(&self, output: &OutputPath) -> PathBuf {
        self.entries
            .get(output)
            .expect("output claims use catalog paths")
            .source
            .clone()
    }

    pub(crate) fn links(
        self: &Arc<Self>,
        locale: Locale,
        base_path: BasePath,
        integrities: BTreeMap<OutputPath, String>,
    ) -> AssetLinks {
        AssetLinks {
            catalog: self.clone(),
            locale,
            base_path,
            integrities: Arc::new(integrities),
        }
    }

    fn resolve_id(&self, locale: &Locale, id: &str) -> Result<ResolvedAsset, ResolveError> {
        let requested = AssetId::parse(id).ok_or(ResolveError::InvalidId)?;
        self.resolve_path(locale, requested)
    }

    fn resolve_relative(
        &self,
        locale: &Locale,
        page_dir: &Path,
        id: &str,
    ) -> Result<ResolvedAsset, ResolveError> {
        let requested = LogicalPath::parse(id).ok_or(ResolveError::InvalidId)?;
        let requested = AssetId::from_path(&page_dir.join(requested.as_path()))
            .expect("page directories and logical asset IDs remain logical");
        self.resolve_path(locale, requested)
    }

    fn resolve_path(
        &self,
        locale: &Locale,
        requested: AssetId,
    ) -> Result<ResolvedAsset, ResolveError> {
        let ids = self
            .locale_entries
            .get(locale)
            .ok_or_else(|| ResolveError::NotFound {
                id: requested.as_path().into(),
            })?;
        if let Some(output) = ids.get(&requested) {
            return Ok(ResolvedAsset {
                id: requested,
                output: output.clone(),
            });
        }
        let Some(version) = self.versions.get(&(locale.clone(), requested.clone())) else {
            return Err(ResolveError::NotFound {
                id: requested.as_path().into(),
            });
        };
        let id = versioned_id(&requested, *version);
        let output = ids
            .get(&id)
            .cloned()
            .ok_or_else(|| ResolveError::NotFound {
                id: requested.as_path().into(),
            })?;
        Ok(ResolvedAsset { id, output })
    }

    fn dimensions(&self, output: &OutputPath) -> Option<(u32, u32)> {
        self.entries.get(output).and_then(|entry| entry.dimensions)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AssetLinks {
    catalog: Arc<AssetCatalog>,
    locale: Locale,
    base_path: BasePath,
    integrities: Arc<BTreeMap<OutputPath, String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AssetLink {
    path: OutputPath,
    url: String,
    integrity: Option<String>,
    dimensions: Option<(u32, u32)>,
}

impl AssetLinks {
    pub(crate) fn with_locale(&self, locale: Locale) -> Self {
        Self {
            catalog: self.catalog.clone(),
            locale,
            base_path: self.base_path.clone(),
            integrities: self.integrities.clone(),
        }
    }

    pub(crate) fn resolve(&self, id: &str) -> Result<AssetLink, ResolveError> {
        let asset = self.catalog.resolve_id(&self.locale, id)?;
        Ok(AssetLink {
            url: self.base_path.join(&asset.output.url_path()),
            integrity: self.integrities.get(&asset.output).cloned(),
            dimensions: self.catalog.dimensions(&asset.output),
            path: asset.output,
        })
    }

    /// A leading `/` names an asset from the current locale root; other IDs
    /// are relative to the current page directory.
    pub(crate) fn resolve_template_path(
        &self,
        page_dir: &Path,
        id: &str,
    ) -> Result<(ResolvedAsset, bool), ResolveError> {
        match id.strip_prefix('/') {
            Some(id) => self
                .catalog
                .resolve_id(&self.locale, id)
                .map(|path| (path, true)),
            None => self
                .catalog
                .resolve_relative(&self.locale, page_dir, id)
                .map(|path| (path, false)),
        }
    }

    pub(crate) fn dimensions(&self, path: &OutputPath) -> Option<(u32, u32)> {
        self.catalog.dimensions(path)
    }
}

impl AssetLink {
    pub(crate) fn path(&self) -> &OutputPath {
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

fn process_versioned(
    locale: &Locale,
    id: &AssetId,
    versions: &mut BTreeMap<(Locale, AssetId), u64>,
) {
    let path = id.as_path();
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
        .entry((
            locale.clone(),
            AssetId::from_path(&key).expect("versioned asset IDs stay logical"),
        ))
        .and_modify(|latest| *latest = (*latest).max(version))
        .or_insert(version);
}

fn versioned_id(path: &AssetId, version: u64) -> AssetId {
    let path = path.as_path();
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
    AssetId::from_path(&path.with_file_name(filename)).expect("versioned asset IDs stay logical")
}
