//! Persistent renderer-result cache and deterministic cache keys.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use super::{
    NamedInput, Output, Profile, RendererOutput, ResultCapabilities, Source, WorkspaceInput,
    validate_renderer_output,
};

/// One build session owns cache cleanup. Complete cache entries remain
/// immutable and available until the site owner removes `.heine/`.
#[derive(Clone, Debug)]
pub(crate) struct RendererCache {
    root: PathBuf,
}

impl RendererCache {
    pub(super) fn open(root: PathBuf) -> Result<Self, String> {
        cleanup_cache(&root)?;
        Ok(Self { root })
    }

    pub(super) fn read(
        &self,
        key: &str,
        profile: &Profile,
        output: Output,
    ) -> Result<Option<RendererOutput>, String> {
        let bytes_path = self.root.join(format!("{key}.bin"));
        let digest_path = self.root.join(format!("{key}.sha256"));
        let (bytes, digest) = match (fs::read(&bytes_path), fs::read_to_string(&digest_path)) {
            (Ok(bytes), Ok(digest)) => (bytes, digest),
            (Err(error), _) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            (_, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            (Err(error), _) => return Err(format!("could not read renderer cache: {error}")),
            (_, Err(error)) => return Err(format!("could not read renderer cache: {error}")),
        };
        let expected = format!(
            "v1 {} {}",
            cache_result_kind(profile.result()),
            hex_digest(&Sha256::digest(&bytes))
        );
        if digest.trim() != expected {
            return Ok(None);
        }
        validate_renderer_output(profile, output, bytes).map(Some)
    }

    pub(super) fn write(
        &self,
        key: &str,
        result: &ResultCapabilities,
        bytes: &[u8],
    ) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("could not create renderer cache: {error}"))?;
        let digest = format!(
            "v1 {} {}\n",
            cache_result_kind(result),
            hex_digest(&Sha256::digest(bytes))
        );
        write_cache_file(&self.root.join(format!("{key}.bin")), bytes)?;
        write_cache_file(&self.root.join(format!("{key}.sha256")), digest.as_bytes())
    }
}

pub(super) enum CacheInput<'a> {
    Body(&'a [u8]),
    Source(&'a Source),
}

pub(super) fn cache_input_for_workspace<'a>(input: &'a WorkspaceInput<'a>) -> CacheInput<'a> {
    match input {
        WorkspaceInput::Body(bytes) => CacheInput::Body(bytes),
        WorkspaceInput::Source(source) => CacheInput::Source(source),
    }
}

pub(super) fn renderer_cache_key(
    protocol: &str,
    profile_name: &str,
    profile: &Profile,
    placement: super::Placement,
    output: Output,
    input: CacheInput<'_>,
) -> Result<String, String> {
    let arguments = match placement {
        super::Placement::Inline => profile.inline.as_ref(),
        super::Placement::Block => profile.block.as_ref(),
    };
    let mut hasher = Sha256::new();
    hasher.update(b"heine-renderer-cache-v2\0");
    for value in [
        profile_name,
        protocol,
        &profile.cache_identity,
        &profile.program,
        profile.input_extension.as_deref().unwrap_or(""),
        match placement {
            super::Placement::Inline => "inline",
            super::Placement::Block => "block",
        },
        match output {
            Output::Embedded => "embedded",
            Output::Resource => "resource",
        },
    ] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher.update(format!("{:?}", profile.result).as_bytes());
    hasher.update([0]);
    for argument in arguments.into_iter().flat_map(|variant| &variant.arguments) {
        hasher.update(argument.as_bytes());
        hasher.update([0]);
    }
    match input {
        CacheInput::Body(bytes) => {
            hasher.update(b"body\0");
            hasher.update(bytes);
        }
        CacheInput::Source(source) => {
            hasher.update(match source.input() {
                NamedInput::File => b"file\0".as_slice(),
                NamedInput::Bundle => b"bundle\0".as_slice(),
            });
            for path in source.files() {
                let relative = path
                    .strip_prefix(source.root())
                    .map_err(|_| "renderer source is outside its checked input root".to_owned())?;
                hasher.update(relative.as_os_str().as_encoded_bytes());
                hasher.update([0]);
                hash_renderer_source(&mut hasher, path)?;
                hasher.update([0]);
            }
        }
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn hash_renderer_source(hasher: &mut Sha256, path: &Path) -> Result<(), String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("could not read renderer source: {error}"))?;
    let mut buffer = [0; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("could not read renderer source: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        hasher.update(&buffer[..read]);
    }
}

fn cleanup_cache(root: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("could not inspect renderer cache: {error}")),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not inspect renderer cache: {error}"))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|error| format!("could not inspect renderer cache: {error}"))?
            .is_file()
        {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let companion = if let Some(key) = name.strip_suffix(".bin") {
            Some(root.join(format!("{key}.sha256")))
        } else {
            name.strip_suffix(".sha256")
                .map(|key| root.join(format!("{key}.bin")))
        };
        if name.contains(".tmp-") || companion.is_some_and(|path| !path.is_file()) {
            fs::remove_file(&path)
                .map_err(|error| format!("could not clean renderer cache: {error}"))?;
        }
    }
    Ok(())
}

fn cache_result_kind(result: &ResultCapabilities) -> &'static str {
    match result {
        ResultCapabilities::Html => "html",
        ResultCapabilities::Mathml => "mathml",
        ResultCapabilities::Svg { .. } => "svg",
        ResultCapabilities::Image { .. } => "image",
        ResultCapabilities::Link { .. } => "link",
    }
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_cache_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not name renderer cache entry: {error}"))?
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("could not write renderer cache: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("could not publish renderer cache: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_cache_cleans_incomplete_entries_once_when_the_session_starts() {
        let root = tempfile::tempdir().expect("cache root");
        let stale = root.path().join("stale.bin");
        fs::write(&stale, "bytes").expect("incomplete cache entry");

        let cache = RendererCache::open(root.path().to_path_buf()).expect("renderer cache");
        assert!(!stale.exists());

        let later_stale = root.path().join("later.bin");
        fs::write(&later_stale, "bytes").expect("later incomplete cache entry");
        cache
            .write("complete", &ResultCapabilities::Html, b"<p>result</p>")
            .expect("cache write");

        assert!(later_stale.exists());
        assert!(root.path().join("complete.bin").is_file());
        assert!(root.path().join("complete.sha256").is_file());
    }

    #[test]
    fn renderer_source_hashing_matches_a_complete_byte_sequence() {
        let root = tempfile::tempdir().expect("source root");
        let source = root.path().join("input.bin");
        let bytes = (0..20_000)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&source, &bytes).expect("renderer source");
        let mut hasher = Sha256::new();

        hash_renderer_source(&mut hasher, &source).expect("streamed source hash");

        assert_eq!(
            hasher.finalize().as_slice(),
            Sha256::digest(&bytes).as_slice()
        );
    }

    #[test]
    fn cache_cleanup_removes_only_incomplete_write_artifacts() {
        let root = tempfile::tempdir().expect("cache root");
        fs::write(root.path().join("complete.bin"), "bytes").expect("complete bytes");
        fs::write(root.path().join("complete.sha256"), "digest").expect("complete digest");
        fs::write(root.path().join("partial.bin"), "bytes").expect("partial bytes");
        fs::write(root.path().join("partial.sha256.tmp-1"), "digest").expect("temporary digest");
        fs::create_dir(root.path().join("directory")).expect("cache directory");

        cleanup_cache(root.path()).expect("cache cleanup");

        assert!(root.path().join("complete.bin").is_file());
        assert!(root.path().join("complete.sha256").is_file());
        assert!(!root.path().join("partial.bin").exists());
        assert!(!root.path().join("partial.sha256.tmp-1").exists());
        assert!(root.path().join("directory").is_dir());
    }
}
