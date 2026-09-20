//! Discovery and locale-aware resolution of copied content assets.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use sha2::Digest;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

use crate::{
    config::{BasePath, CanonicalUrls, IntegrityAlgorithm},
    locale::Locale,
    output::{CopiedFiles, OutputPath},
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
    #[error("could not walk kit asset directory {path}")]
    KitWalk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("kit asset directory {path} is missing or is not a directory")]
    MissingKitDirectory { path: PathBuf },
    #[error("selected kit manifest source was unavailable while validating its assets")]
    MissingKitManifestSource,
    #[error("kit asset path {path} is a symbolic link")]
    KitSymlink { path: PathBuf },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Kit(#[from] Box<KitAssetDiagnostic>),
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
    #[error("asset {asset_path} would be published to both {first} and {second}")]
    SourceOutputConflict {
        asset_path: PathBuf,
        first: PathBuf,
        second: PathBuf,
    },
    #[error("asset catalog has no entry for source {path}")]
    UnknownSource { path: PathBuf },
    #[error("asset catalog has no entry for output {path}")]
    UnknownOutput { path: PathBuf },
    #[error("asset catalog already has licensing for output {path}")]
    ExistingLicensing { path: PathBuf },
}

#[derive(Debug, Diagnostic, Error)]
#[error("invalid public kit asset declaration: {message}")]
#[diagnostic(code(heine::kit::asset))]
pub(crate) struct KitAssetDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("declared public asset here")]
    span: SourceSpan,
}

#[derive(Debug, Error)]
pub(crate) enum ResolveError {
    #[error(
        "asset id must be a non-empty path relative to the current locale content root; it must not start with '/', contain '.', '..', or backslashes"
    )]
    InvalidId,
    #[error("asset {id:?} was not found")]
    NotFound { id: PathBuf },
    #[error("asset {id:?} is private to its kit; only kit templates can address it")]
    PrivateKitAsset { id: String },
}

/// A locale source tree plus its optional shared-asset fallback.
pub(crate) struct AssetSources<'a> {
    pub(crate) root: &'a Path,
    pub(crate) entries: &'a [DirEntry],
    pub(crate) content_inputs: &'a BTreeSet<PathBuf>,
    pub(crate) output_prefix: &'a Path,
    pub(crate) shared: Option<SharedAssets<'a>>,
    pub(crate) kit: Option<&'a KitAssets>,
}

pub(crate) struct SharedAssets<'a> {
    pub(crate) root: &'a Path,
    pub(crate) entries: &'a [DirEntry],
}

/// Selected kit assets prepared for the same locale-aware inventory used by
/// root assets. The manifest determines which physical paths become public.
pub(crate) struct KitAssets {
    root: PathBuf,
    entries: Vec<DirEntry>,
    name: crate::kit::KitName,
    public: BTreeMap<PathBuf, LogicalPath>,
}

#[derive(Debug, Clone)]
struct AssetEntry {
    source: PathBuf,
    dimensions: Option<(u32, u32)>,
    licensing: Option<Arc<AssetLicensing>>,
    layer: AssetLayer,
}

/// Licensing metadata attached to one copied asset after its REUSE sidecar is
/// validated. Output links remain paths here; URLs are a template concern.
#[derive(Debug, Clone)]
pub(crate) struct AssetLicensing {
    pub(crate) copyrights: Vec<String>,
    pub(crate) expressions: Vec<String>,
    pub(crate) comments: Vec<String>,
    pub(crate) license_texts: Vec<LicenseText>,
}

#[derive(Debug, Clone)]
pub(crate) struct LicenseText {
    pub(crate) id: String,
    pub(crate) output: OutputPath,
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

impl KitAssets {
    pub(crate) fn load(selected: &crate::kit::Selected) -> Result<Self, Error> {
        let root = selected.paths().assets();
        let metadata = match fs::symlink_metadata(root) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(Error::MissingKitDirectory { path: root.into() });
            }
            Err(source) => {
                return Err(Error::Io {
                    path: root.into(),
                    source,
                });
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(Error::KitSymlink { path: root.into() });
        }
        if !metadata.is_dir() {
            return Err(Error::MissingKitDirectory { path: root.into() });
        }

        let entries = WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .map(|entry| {
                entry.map_err(|source| Error::KitWalk {
                    path: root.into(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for entry in &entries {
            if entry.file_type().is_symlink() {
                return Err(Error::KitSymlink {
                    path: entry.path().into(),
                });
            }
        }

        let files = entries
            .iter()
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_none_or(|extension| extension != "license")
            })
            .map(|entry| {
                let relative = relative_path(root, entry.path())?;
                if !source_path::has_safe_components(&relative) {
                    return Err(Error::NonUtf8Path {
                        path: entry.path().into(),
                    });
                }
                Ok(relative)
            })
            .collect::<Result<BTreeSet<_>, Error>>()?;

        let source_code = selected
            .manifest()
            .source()
            .cloned()
            .ok_or(Error::MissingKitManifestSource)?;
        let mut public = BTreeMap::new();
        for declaration in selected.manifest().public().assets() {
            let physical = declaration
                .value()
                .as_path()
                .strip_prefix("assets")
                .map_err(|_| {
                    public_asset_error(
                        declaration,
                        source_code.clone(),
                        "public asset paths must begin with \"assets/\"",
                    )
                })?
                .to_path_buf();
            if !files.contains(&physical) {
                return Err(public_asset_error(
                    declaration,
                    source_code.clone(),
                    format!(
                        "declared asset {:?} does not exist below the kit assets directory",
                        declaration.value()
                    ),
                ));
            }
            public.insert(physical, declaration.value().clone());
        }

        Ok(Self {
            root: root.into(),
            entries,
            name: selected.manifest().name().clone(),
            public,
        })
    }

    fn entries(&self) -> &[DirEntry] {
        &self.entries
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn asset_id(&self, relative: &Path) -> Result<(AssetId, AssetLayer), Error> {
        if let Some(public) = self.public.get(relative) {
            return AssetId::from_path(public.as_path())
                .map(|id| (id, AssetLayer::KitPublic))
                .ok_or_else(|| Error::NonUtf8Path {
                    path: self.root.join(relative),
                });
        }
        let private = Path::new("kit")
            .join(self.name.as_str())
            .join("assets")
            .join(relative);
        AssetId::from_path(&private)
            .map(|id| (id, AssetLayer::KitPrivate))
            .ok_or_else(|| Error::NonUtf8Path {
                path: self.root.join(relative),
            })
    }
}

fn public_asset_error(
    declaration: &crate::kit::Declaration<LogicalPath>,
    source_code: NamedSource<String>,
    message: impl Into<String>,
) -> Error {
    Box::new(KitAssetDiagnostic {
        message: message.into(),
        source_code,
        span: declaration.span(),
    })
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AssetLayer {
    KitPrivate,
    KitPublic,
    RootShared,
    RootLocale,
}

/// Validated inventory of copied assets. Logical IDs resolve per locale;
/// output paths remain global for the output domain.
#[derive(Debug, Clone)]
pub(crate) struct AssetCatalog {
    entries: BTreeMap<OutputPath, AssetEntry>,
    source_outputs: BTreeMap<PathBuf, OutputPath>,
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
                kit: None,
            },
        )]);
        Self::discover_locales(&sources)
    }

    pub(crate) fn discover_locales(
        sources: &BTreeMap<Locale, AssetSources<'_>>,
    ) -> Result<Self, Error> {
        let mut catalog = Self {
            entries: BTreeMap::new(),
            source_outputs: BTreeMap::new(),
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
        if let Some(kit) = sources.kit {
            for entry in kit.entries() {
                if !entry.file_type().is_file()
                    || entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "license")
                {
                    continue;
                }
                let relative = relative_path(kit.root(), entry.path())?;
                let (id, layer) = kit.asset_id(&relative)?;
                let output = OutputPath::try_new(id.as_path().into()).map_err(|message| {
                    Error::InvalidOutputPath {
                        asset_path: entry.path().into(),
                        output: id.as_path().into(),
                        message,
                    }
                })?;
                self.add_entry(locale, &mut ids, id, output, entry.path(), layer)?;
            }
        }
        if let Some(shared) = &sources.shared {
            for entry in shared.entries {
                if entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_none_or(|extension| extension != "license")
                {
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
                    self.add_entry(
                        locale,
                        &mut ids,
                        id.clone(),
                        output,
                        entry.path(),
                        AssetLayer::RootShared,
                    )?;
                }
            }
        }
        for entry in sources.entries {
            if !entry.file_type().is_file()
                || entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "page" || extension == "license")
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
            self.add_entry(
                locale,
                &mut ids,
                id.clone(),
                output,
                entry.path(),
                AssetLayer::RootLocale,
            )?;
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
        layer: AssetLayer,
    ) -> Result<(), Error> {
        if !source_path::has_safe_components(id.as_path())
            || !source_path::has_safe_components(output.as_path())
        {
            return Err(Error::NonUtf8Path {
                path: source.into(),
            });
        }
        process_versioned(locale, &id, &mut self.versions);
        if let Some(existing) = self.entries.get(&output) {
            // Shared and kit assets participate in every locale inventory. A
            // source already recorded for this output needs only this locale's
            // logical lookup entry, not another metadata read or image decode.
            if existing.source == source {
                ids.insert(id, output);
                return Ok(());
            }
            if existing.layer == AssetLayer::KitPrivate
                || layer == AssetLayer::KitPrivate
                || (existing.layer != AssetLayer::KitPublic && layer != AssetLayer::KitPublic)
            {
                return Err(Error::OutputCollision {
                    output: output.as_path().into(),
                    first: existing.source.clone(),
                    second: source.into(),
                });
            }
            if existing.layer > layer {
                ids.insert(id, output);
                return Ok(());
            }
            self.source_outputs.remove(&existing.source);
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
        let dimensions = is_decodable_image(id.as_path())
            .then(|| {
                image::ImageReader::open(source)
                    .ok()?
                    .into_dimensions()
                    .ok()
            })
            .flatten();
        self.entries.insert(
            output.clone(),
            AssetEntry {
                source: source.into(),
                dimensions,
                licensing: None,
                layer,
            },
        );
        match self.source_outputs.entry(source.into()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(output.clone());
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &output => {
                return Err(Error::SourceOutputConflict {
                    asset_path: source.into(),
                    first: entry.get().as_path().into(),
                    second: output.as_path().into(),
                });
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
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

    pub(crate) fn source_path(&self, output: &OutputPath) -> &Path {
        &self
            .entries
            .get(output)
            .expect("output claims use catalog paths")
            .source
    }

    pub(crate) fn contains_source(&self, source: &Path) -> bool {
        self.source_outputs.contains_key(source)
    }

    pub(crate) fn contains_output(&self, output: &OutputPath) -> bool {
        self.entries.contains_key(output)
    }

    pub(crate) fn attach_licensing(
        &mut self,
        source: &Path,
        licensing: AssetLicensing,
        modified: SystemTime,
    ) -> Result<OutputPath, Error> {
        let Some(output) = self.source_outputs.get(source).cloned() else {
            return Err(Error::UnknownSource {
                path: source.into(),
            });
        };
        let Some(entry) = self.entries.get_mut(&output) else {
            return Err(Error::UnknownSource {
                path: source.into(),
            });
        };
        if entry.licensing.is_some() {
            return Err(Error::ExistingLicensing {
                path: output.as_path().into(),
            });
        }
        entry.licensing = Some(Arc::new(licensing));
        self.latest_input = Some(
            self.latest_input
                .map_or(modified, |latest| latest.max(modified)),
        );
        Ok(output)
    }

    pub(crate) fn attach_output_licensing(
        &mut self,
        output: &OutputPath,
        licensing: AssetLicensing,
    ) -> Result<(), Error> {
        let Some(entry) = self.entries.get_mut(output) else {
            return Err(Error::UnknownOutput {
                path: output.as_path().into(),
            });
        };
        if entry.licensing.is_some() {
            return Err(Error::ExistingLicensing {
                path: output.as_path().into(),
            });
        }
        entry.licensing = Some(Arc::new(licensing));
        Ok(())
    }

    pub(crate) fn unlicensed_assets(&self) -> Vec<(OutputPath, PathBuf)> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.licensing.is_none())
            .map(|(output, entry)| (output.clone(), entry.source.clone()))
            .collect()
    }

    pub(crate) fn links(
        self: &Arc<Self>,
        locale: Locale,
        base_path: BasePath,
        canonical_urls: Option<CanonicalUrls>,
        integrities: BTreeMap<OutputPath, String>,
    ) -> AssetLinks {
        AssetLinks {
            catalog: self.clone(),
            locale,
            base_path,
            canonical_urls,
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

    fn is_private_kit_asset(&self, output: &OutputPath) -> bool {
        self.entries
            .get(output)
            .is_some_and(|entry| entry.layer == AssetLayer::KitPrivate)
    }

    fn licensing(&self, output: &OutputPath) -> Option<&Arc<AssetLicensing>> {
        self.entries
            .get(output)
            .and_then(|entry| entry.licensing.as_ref())
    }
}

impl CopiedFiles for AssetCatalog {
    fn paths(&self) -> &[OutputPath] {
        self.paths()
    }

    fn source_path(&self, output: &OutputPath) -> &Path {
        self.source_path(output)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AssetLinks {
    catalog: Arc<AssetCatalog>,
    locale: Locale,
    base_path: BasePath,
    canonical_urls: Option<CanonicalUrls>,
    integrities: Arc<BTreeMap<OutputPath, String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AssetLink {
    path: OutputPath,
    url: String,
    absolute_url: Option<String>,
    integrity: Option<String>,
    dimensions: Option<(u32, u32)>,
    image: bool,
    licensing: Option<Arc<AssetLicensing>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AssetLicensingView {
    copyrights: Vec<String>,
    expressions: Vec<String>,
    comments: Vec<String>,
    license_texts: Vec<AssetLicenseTextView>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AssetLicenseTextView {
    id: String,
    url: String,
    relative_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    absolute_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AttributionView {
    asset: AssetAttributionAssetView,
    licensing: AssetLicensingView,
}

#[derive(Debug, Serialize)]
pub(crate) struct AssetAttributionAssetView {
    output_path: String,
    url: String,
    relative_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    absolute_url: Option<String>,
}

impl AssetLinks {
    pub(crate) fn base_path(&self) -> &BasePath {
        &self.base_path
    }

    pub(crate) fn canonical_urls(&self) -> Option<&CanonicalUrls> {
        self.canonical_urls.as_ref()
    }
    pub(crate) fn with_locale(&self, locale: Locale) -> Self {
        Self {
            catalog: self.catalog.clone(),
            locale,
            base_path: self.base_path.clone(),
            canonical_urls: self.canonical_urls.clone(),
            integrities: self.integrities.clone(),
        }
    }

    /// Resolves the public asset surface shared by Markdown and root
    /// templates. Private kit assets retain output identities, but only the
    /// kit's rewritten `asset()` calls may use the wider resolver below.
    pub(crate) fn resolve(&self, id: &str) -> Result<AssetLink, ResolveError> {
        self.resolve_with_private_assets(id, false)
    }

    pub(crate) fn resolve_for_kit(&self, id: &str) -> Result<AssetLink, ResolveError> {
        self.resolve_with_private_assets(id, true)
    }

    fn resolve_with_private_assets(
        &self,
        id: &str,
        include_private_assets: bool,
    ) -> Result<AssetLink, ResolveError> {
        let asset = self.catalog.resolve_id(&self.locale, id)?;
        if !include_private_assets && self.catalog.is_private_kit_asset(&asset.output) {
            return Err(ResolveError::PrivateKitAsset { id: id.into() });
        }
        Ok(AssetLink {
            url: self.base_path.join(&asset.output.url_path()),
            absolute_url: self
                .canonical_urls
                .as_ref()
                .map(|urls| urls.output_url(&asset.output)),
            integrity: self.integrities.get(&asset.output).cloned(),
            dimensions: self.catalog.dimensions(&asset.output),
            image: is_image_media(asset.output.as_path()),
            licensing: self.catalog.licensing(&asset.output).cloned(),
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

    pub(crate) fn attributions(&self, root: &str) -> Vec<AttributionView> {
        self.catalog
            .entries
            .iter()
            .filter_map(|(path, entry)| {
                let licensing = entry.licensing.as_ref()?;
                Some(AttributionView {
                    asset: AssetAttributionAssetView {
                        output_path: path.logical_spelling(),
                        url: self.base_path.join(&path.url_path()),
                        relative_url: format!("{root}{}", path.url_path().as_str()),
                        absolute_url: self
                            .canonical_urls
                            .as_ref()
                            .map(|urls| urls.output_url(path)),
                    },
                    licensing: licensing.view(&self.base_path, root, self.canonical_urls.as_ref()),
                })
            })
            .collect()
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
    pub(crate) fn absolute_url(&self) -> Option<&str> {
        self.absolute_url.as_deref()
    }
    pub(crate) fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }
    pub(crate) fn is_image(&self) -> bool {
        self.image
    }
    pub(crate) fn licensing_view(
        &self,
        root: &str,
        base_path: &BasePath,
        canonical_urls: Option<&CanonicalUrls>,
    ) -> Option<AssetLicensingView> {
        self.licensing
            .as_ref()
            .map(|licensing| licensing.view(base_path, root, canonical_urls))
    }
}

impl AssetLicensing {
    fn view(
        &self,
        base_path: &BasePath,
        root: &str,
        canonical_urls: Option<&CanonicalUrls>,
    ) -> AssetLicensingView {
        AssetLicensingView {
            copyrights: self.copyrights.clone(),
            expressions: self.expressions.clone(),
            comments: self.comments.clone(),
            license_texts: self
                .license_texts
                .iter()
                .map(|license| AssetLicenseTextView {
                    id: license.id.clone(),
                    url: base_path.join(&license.output.url_path()),
                    relative_url: format!("{root}{}", license.output.url_path().as_str()),
                    absolute_url: canonical_urls.map(|urls| urls.output_url(&license.output)),
                })
                .collect(),
        }
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

fn is_decodable_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpeg" | "jpg" | "webp" | "png" | "gif"
            )
        })
}

fn is_image_media(path: &Path) -> bool {
    mime_guess::from_path(path)
        .first_raw()
        .is_some_and(|mime| mime.starts_with("image/"))
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
