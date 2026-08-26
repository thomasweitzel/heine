//! Output-tree materialization for full and quick builds.

use super::{
    Error, GeneratedFile, is_internal_output_path,
    publication::StagingDirectory,
    target::{OutputDirectory, OutputTarget, ensure_directory, ensure_parent, write_marker},
};
use crate::{asset::AssetCatalog, site::BuildMode};
use std::{fs, io, path::Path};

pub(crate) struct OutputWriter {
    target: OutputTarget,
}
impl OutputWriter {
    pub(crate) fn new(target: OutputTarget) -> Self {
        Self { target }
    }
    pub(crate) fn write(
        &self,
        assets: &AssetCatalog,
        generated_files: impl IntoIterator<Item = GeneratedFile>,
        mode: BuildMode,
        clean_output: bool,
    ) -> Result<(), Error> {
        self.target.ensure_no_symlinked_ancestor()?;
        ensure_marker_is_available(assets)?;
        match mode {
            BuildMode::Full => self.write_full(assets, generated_files, clean_output),
            BuildMode::Quick => self.write_quick(assets, generated_files),
        }
    }
    fn write_full(
        &self,
        assets: &AssetCatalog,
        generated_files: impl IntoIterator<Item = GeneratedFile>,
        clean_output: bool,
    ) -> Result<(), Error> {
        let output = OutputDirectory::inspect(self.target.root())?;
        output.require_replacement_permission(clean_output)?;
        let mut staging = StagingDirectory::create(self.target.root())?;
        write_tree(staging.path(), assets, generated_files)?;
        write_marker(staging.path())?;
        staging.publish(output.exists())
    }
    fn write_quick(
        &self,
        assets: &AssetCatalog,
        generated_files: impl IntoIterator<Item = GeneratedFile>,
    ) -> Result<(), Error> {
        let output = OutputDirectory::inspect(self.target.root())?;
        output.require_update_permission()?;
        ensure_directory(self.target.root())?;
        if !output.exists() || output.is_empty() {
            write_marker(self.target.root())?;
        }
        copy_newer_assets(self.target.root(), assets)?;
        write_generated_files(
            self.target.root(),
            generated_files,
            WriteSafety::ExistingOutput,
        )
    }
}

fn write_tree(
    root: &Path,
    assets: &AssetCatalog,
    generated_files: impl IntoIterator<Item = GeneratedFile>,
) -> Result<(), Error> {
    copy_assets(root, assets)?;
    write_generated_files(root, generated_files, WriteSafety::Staging)
}
fn ensure_marker_is_available(assets: &AssetCatalog) -> Result<(), Error> {
    if let Some(asset) = assets
        .paths()
        .iter()
        .find(|asset| is_internal_output_path(asset.as_path()))
    {
        return Err(Error::ReservedPath {
            path: asset.as_path().into(),
        });
    }
    Ok(())
}
fn copy_assets(root: &Path, assets: &AssetCatalog) -> Result<(), Error> {
    for asset in assets.paths() {
        copy_asset(&assets.source_path(asset), &root.join(asset.as_path()))?;
    }
    Ok(())
}
fn copy_newer_assets(root: &Path, assets: &AssetCatalog) -> Result<(), Error> {
    for asset in assets.paths() {
        let source = assets.source_path(asset);
        let target = root.join(asset.as_path());
        ensure_safe_output_target(root, asset.as_path())?;
        if asset_needs_copying(&source, &target)? {
            copy_asset(&source, &target)?;
        }
    }
    Ok(())
}
fn asset_needs_copying(source: &Path, target: &Path) -> Result<bool, Error> {
    let source_modified = fs::metadata(source)
        .and_then(|metadata| metadata.modified())
        .map_err(|source_error| Error::AssetMetadata {
            path: source.into(),
            source: source_error,
        })?;
    match fs::metadata(target).and_then(|metadata| metadata.modified()) {
        Ok(target_modified) => Ok(source_modified > target_modified),
        Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(source_error) => Err(Error::AssetMetadata {
            path: target.into(),
            source: source_error,
        }),
    }
}
fn copy_asset(source: &Path, target: &Path) -> Result<(), Error> {
    ensure_parent(target)?;
    fs::copy(source, target).map_err(|source_error| Error::AssetCopy {
        source_path: source.into(),
        output_path: target.into(),
        source: source_error,
    })?;
    Ok(())
}
fn write_generated_files(
    root: &Path,
    generated_files: impl IntoIterator<Item = GeneratedFile>,
    safety: WriteSafety,
) -> Result<(), Error> {
    for file in generated_files {
        if safety == WriteSafety::ExistingOutput {
            ensure_safe_output_target(root, file.path.as_path())?;
        }
        let target = root.join(file.path.as_path());
        ensure_parent(&target)?;
        fs::write(&target, file.bytes).map_err(|source| Error::Io {
            path: target,
            source,
        })?;
    }
    Ok(())
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteSafety {
    Staging,
    ExistingOutput,
}
fn ensure_safe_output_target(root: &Path, relative: &Path) -> Result<(), Error> {
    let mut target = root.to_path_buf();
    for component in relative.components() {
        target.push(component.as_os_str());
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::OutputTargetSymlink { path: target });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(source) => {
                return Err(Error::Io {
                    path: target,
                    source,
                });
            }
        }
    }
    Ok(())
}
