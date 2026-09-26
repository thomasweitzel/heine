//! Validation for site-owned renderer-profile configuration.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use miette::SourceSpan;
use quick_xml::{Reader, Writer, events::Event};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::source_path;

#[derive(Debug, Clone, Default)]
pub(crate) struct Profiles {
    profiles: BTreeMap<String, Profile>,
}

impl Profiles {
    pub(crate) fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub(crate) fn uses_workspace(&self) -> bool {
        self.profiles.values().any(Profile::uses_workspace)
    }
}

pub(crate) fn parse(
    profiles: BTreeMap<toml::Spanned<String>, RawProfile>,
) -> Result<Profiles, ConfigError> {
    let mut parsed = BTreeMap::new();
    for (raw_name, profile) in profiles {
        let span = raw_name.span().into();
        let name = raw_name.into_inner();
        if !source_path::is_simple_identifier(&name) {
            return Err(ConfigError {
                span,
                message: format!("renderer profile name {name:?} must be a simple identifier"),
            });
        }
        parsed.insert(name.clone(), validate_profile(&name, profile)?);
    }
    Ok(Profiles { profiles: parsed })
}

fn validate_profile(name: &str, profile: RawProfile) -> Result<Profile, ConfigError> {
    let RawProfile {
        protocol,
        program,
        timeout_seconds,
        cache_identity,
        inputs,
        result,
        input_extension,
        media_type,
        extension,
        materializations,
        default_output,
        inline,
        block,
    } = profile;
    let program = validate_nonempty(program, &format!("renderer profile {name} program"))?;
    let cache_identity = validate_nonempty(
        cache_identity,
        &format!("renderer profile {name} cache_identity"),
    )?;
    let timeout_span = timeout_seconds.span().into();
    let timeout_seconds = timeout_seconds.into_inner();
    if timeout_seconds == 0 {
        return Err(ConfigError {
            span: timeout_span,
            message: format!("renderer profile {name} timeout_seconds must be positive"),
        });
    }
    let inputs_span = inputs.span().into();
    let inputs = parse_inputs(name, inputs)?;
    if protocol == Protocol::Stream && inputs.contains(&Input::Bundle) {
        return Err(ConfigError {
            span: inputs_span,
            message: format!(
                "stream renderer profile {name} may accept only \"body\" and \"file\" inputs"
            ),
        });
    }
    let input_extension =
        validate_input_extension(name, protocol, &inputs, input_extension, inputs_span)?;
    let result = validate_result(
        name,
        result,
        ResultOptions {
            media_type,
            extension,
            materializations,
            default_output,
        },
    )?;
    let inline = inline
        .map(|variant| validate_variant(name, "inline", protocol, &inputs, variant))
        .transpose()?;
    let block = block
        .map(|variant| validate_variant(name, "block", protocol, &inputs, variant))
        .transpose()?;
    if inline.is_none() && block.is_none() {
        return Err(ConfigError {
            span: inputs_span,
            message: format!(
                "renderer profile {name} must declare an inline or block placement variant"
            ),
        });
    }
    Ok(Profile {
        protocol,
        program,
        timeout_seconds,
        cache_identity,
        input_extension,
        inputs,
        result,
        inline,
        block,
    })
}

// Execution consumes these values after source resolution has established the
// request inventory. Keeping them here prevents configuration from being
// reparsed or reconstructed at the process boundary.
#[derive(Debug, Clone)]
pub(crate) struct Profile {
    protocol: Protocol,
    program: String,
    timeout_seconds: u64,
    cache_identity: String,
    input_extension: Option<String>,
    inputs: BTreeSet<Input>,
    result: ResultCapabilities,
    inline: Option<Variant>,
    block: Option<Variant>,
}

impl Profile {
    pub(crate) fn supports(&self, placement: Placement) -> bool {
        match placement {
            Placement::Inline => self.inline.is_some(),
            Placement::Block => self.block.is_some(),
        }
    }

    pub(crate) fn accepts_body(&self) -> bool {
        self.inputs.contains(&Input::Body)
    }

    pub(crate) fn accepts_named_source(&self, source: NamedInput) -> bool {
        self.inputs.contains(&match source {
            NamedInput::File => Input::File,
            NamedInput::Bundle => Input::Bundle,
        })
    }

    pub(crate) fn result(&self) -> &ResultCapabilities {
        &self.result
    }

    pub(crate) fn uses_workspace(&self) -> bool {
        self.protocol == Protocol::Workspace
    }

    pub(crate) fn resource_extension(&self) -> Option<&str> {
        match &self.result {
            ResultCapabilities::Svg { .. } => Some("svg"),
            ResultCapabilities::Image { extension } | ResultCapabilities::Link { extension } => {
                Some(extension)
            }
            ResultCapabilities::Html | ResultCapabilities::Mathml => None,
        }
    }

    fn workspace_variant(&self, placement: Placement) -> Result<&Variant, String> {
        if self.protocol != Protocol::Workspace {
            return Err("selected profile does not use the workspace protocol".into());
        }
        match placement {
            Placement::Inline => self.inline.as_ref(),
            Placement::Block => self.block.as_ref(),
        }
        .ok_or_else(|| "selected profile does not support this placement".to_owned())
    }
}

/// A checked input staged into one renderer-owned workspace.
pub(crate) enum WorkspaceInput<'a> {
    Body(&'a [u8]),
    Source(&'a Source),
}

/// Everything the host has already checked for one workspace renderer request.
/// Grouping these values keeps the execution boundary explicit as it grows.
pub(crate) struct WorkspaceRequest<'a> {
    pub(crate) runtime: &'a WorkspaceRuntime,
    pub(crate) cache_root: &'a Path,
    pub(crate) refresh: bool,
    pub(crate) profile_name: &'a str,
    pub(crate) profile: &'a Profile,
    pub(crate) placement: Placement,
    pub(crate) output: Output,
    pub(crate) input: WorkspaceInput<'a>,
}

/// State that outlives one build when the development server rebuilds a site.
/// Stream-only configurations do not allocate a temporary directory.
#[derive(Clone, Debug)]
pub(crate) struct WorkspaceRuntime {
    manager: Option<Arc<Mutex<WorkspaceManager>>>,
}

impl WorkspaceRuntime {
    pub(crate) fn new(profiles: &Profiles) -> Result<Self, String> {
        let manager = profiles
            .uses_workspace()
            .then(WorkspaceManager::new)
            .transpose()?
            .map(|manager| Arc::new(Mutex::new(manager)));
        Ok(Self { manager })
    }
}

const WORKSPACE_SESSION_PREFIX: &str = "heine-renderer-session-";

/// A synchronous renderer session has one reusable slot per profile. Keeping
/// the manager locked for a request preserves that bound if a future caller
/// renders pages from more than one thread.
#[derive(Debug)]
struct WorkspaceManager {
    session: tempfile::TempDir,
    slots: BTreeMap<String, WorkspaceSlot>,
}

#[derive(Debug)]
struct WorkspaceSlot {
    path: PathBuf,
    available: bool,
}

impl WorkspaceManager {
    fn new() -> Result<Self, String> {
        let temporary_directory = std::env::temp_dir();
        static STALE_WORKSPACE_SWEEP: OnceLock<()> = OnceLock::new();
        STALE_WORKSPACE_SWEEP
            .get_or_init(|| cleanup_stale_workspace_sessions(&temporary_directory));
        Self::create_in(&temporary_directory)
    }

    #[cfg(test)]
    fn new_in(temporary_directory: &Path) -> Result<Self, String> {
        cleanup_stale_workspace_sessions(temporary_directory);
        Self::create_in(temporary_directory)
    }

    fn create_in(temporary_directory: &Path) -> Result<Self, String> {
        let session = tempfile::Builder::new()
            .prefix(WORKSPACE_SESSION_PREFIX)
            .tempdir_in(temporary_directory)
            .map_err(|error| format!("could not create renderer workspace session: {error}"))?;
        Ok(Self {
            session,
            slots: BTreeMap::new(),
        })
    }

    fn prepare(&mut self, profile_name: &str) -> Result<PathBuf, String> {
        let slot = self
            .slots
            .entry(profile_name.to_owned())
            .or_insert_with(|| WorkspaceSlot {
                path: self.session.path().join(profile_name),
                available: true,
            });
        let path = slot.path.clone();
        let was_unavailable = !slot.available;
        self.clean(profile_name).map_err(|error| {
            if was_unavailable {
                format!(
                    "renderer workspace slot for {profile_name:?} is still unavailable: {error}"
                )
            } else {
                error
            }
        })?;
        if let Err(error) = fs::create_dir(&path) {
            self.slots
                .get_mut(profile_name)
                .expect("workspace slot is registered before preparation")
                .available = false;
            return Err(format!(
                "could not prepare renderer workspace {}: {error}",
                path.display()
            ));
        }
        Ok(path)
    }

    fn clean(&mut self, profile_name: &str) -> Result<(), String> {
        let slot = self
            .slots
            .get_mut(profile_name)
            .expect("workspace slot is registered before cleanup");
        match fs::remove_dir_all(&slot.path) {
            Ok(()) => {
                slot.available = true;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                slot.available = true;
                Ok(())
            }
            Err(error) => {
                slot.available = false;
                Err(format!(
                    "could not clean renderer workspace {}; it remains unavailable: {error}",
                    slot.path.display()
                ))
            }
        }
    }
}

fn cleanup_stale_workspace_sessions(temporary_directory: &Path) {
    let Ok(entries) = fs::read_dir(temporary_directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(WORKSPACE_SESSION_PREFIX) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Runs one workspace profile in an isolated temporary directory and returns
/// only its declared primary result. The workspace itself is never visible to
/// the renderer as a project path.
pub(crate) fn run_workspace(request: WorkspaceRequest<'_>) -> Result<Vec<u8>, String> {
    let WorkspaceRequest {
        runtime,
        cache_root,
        refresh,
        profile_name,
        profile,
        placement,
        output,
        input,
    } = request;
    let cache_key = renderer_cache_key(
        "workspace",
        profile_name,
        profile,
        placement,
        output,
        cache_input_for_workspace(&input)?,
    )?;
    if !refresh {
        if let Some(bytes) = read_cache(cache_root, &cache_key, profile.result())? {
            return validate_renderer_output(profile.result(), output, bytes);
        }
    }
    let variant = profile.workspace_variant(placement)?;
    let manager = runtime
        .manager
        .as_ref()
        .ok_or_else(|| "workspace renderer runtime is not available".to_owned())?;
    let mut manager = manager
        .lock()
        .map_err(|_| "renderer workspace runtime is unavailable".to_owned())?;
    let workspace = manager.prepare(profile_name)?;
    let result = (|| {
        let layout = WorkspaceLayout::create(&workspace, profile, input)?;
        let arguments = expand_workspace_arguments(&variant.arguments, &layout);
        execute_workspace_process(profile, &arguments, &layout)?;
        validate_renderer_output(
            profile.result(),
            output,
            validate_workspace_output(&layout)?,
        )
    })();
    let cleanup = manager.clean(profile_name);
    let result = match (result, cleanup) {
        (Ok(result), Ok(())) => result,
        // A complete result remains usable when a platform lock delays cleanup.
        // The unavailable slot is retried before the next request.
        (Ok(result), Err(cleanup)) => {
            eprintln!("renderer workspace cleanup warning: {cleanup}");
            result
        }
        (Err(error), Err(cleanup)) => {
            eprintln!("renderer workspace cleanup warning: {cleanup}");
            return Err(error);
        }
        (Err(error), Ok(())) => return Err(error),
    };
    write_cache(cache_root, &cache_key, profile.result(), &result)?;
    Ok(result)
}

struct WorkspaceLayout {
    input_dir: PathBuf,
    input_file: PathBuf,
    output_dir: PathBuf,
    output_file: PathBuf,
    work_dir: PathBuf,
}

impl WorkspaceLayout {
    fn create(root: &Path, profile: &Profile, input: WorkspaceInput<'_>) -> Result<Self, String> {
        let input_dir = root.join("input");
        let output_dir = root.join("output");
        let work_dir = root.join("work");
        for path in [&input_dir, &output_dir, &work_dir] {
            fs::create_dir(path)
                .map_err(|error| format!("could not create renderer workspace: {error}"))?;
        }
        let input_file = match input {
            WorkspaceInput::Body(bytes) => {
                let extension = profile.input_extension.as_deref().ok_or_else(|| {
                    "workspace profile does not declare an input extension".to_owned()
                })?;
                let path = input_dir.join(format!("input.{extension}"));
                fs::write(&path, bytes)
                    .map_err(|error| format!("could not stage renderer body input: {error}"))?;
                path
            }
            WorkspaceInput::Source(source) => {
                for file in source.files() {
                    let relative = file.strip_prefix(source.root()).map_err(|_| {
                        "renderer source is outside its checked input root".to_owned()
                    })?;
                    let destination = input_dir.join(relative);
                    let parent = destination.parent().expect("staged file has a parent");
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("could not stage renderer source: {error}"))?;
                    fs::copy(file, &destination)
                        .map_err(|error| format!("could not stage renderer source: {error}"))?;
                }
                input_dir.join(source.entry().strip_prefix(source.root()).map_err(|_| {
                    "renderer source entry is outside its checked input root".to_owned()
                })?)
            }
        };
        let output_file = output_dir.join("result");
        Ok(Self {
            input_dir,
            input_file,
            output_dir,
            output_file,
            work_dir,
        })
    }
}

fn expand_workspace_arguments(arguments: &[String], layout: &WorkspaceLayout) -> Vec<PathBuf> {
    arguments
        .iter()
        .map(|argument| match argument.as_str() {
            "{input_file}" => layout.input_file.clone(),
            "{input_dir}" => layout.input_dir.clone(),
            "{output_file}" => layout.output_file.clone(),
            "{work_dir}" => layout.work_dir.clone(),
            _ => PathBuf::from(argument),
        })
        .collect()
}

fn execute_workspace_process(
    profile: &Profile,
    arguments: &[PathBuf],
    layout: &WorkspaceLayout,
) -> Result<(), String> {
    let mut child = Command::new(&profile.program)
        .args(arguments)
        .current_dir(&layout.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start renderer: {error}"))?;
    let mut stderr = child.stderr.take().expect("piped renderer stderr");
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(profile.timeout_seconds);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not wait for renderer: {error}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "renderer exceeded its {}-second timeout",
                profile.timeout_seconds
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stderr = stderr_reader
        .join()
        .map_err(|_| "renderer stderr reader panicked".to_owned())
        .and_then(|result| {
            result.map_err(|error| format!("could not read renderer diagnostics: {error}"))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "renderer exited with {}; {}",
            status,
            String::from_utf8_lossy(&stderr[..stderr.len().min(8192)])
        ))
    }
}

fn validate_workspace_output(layout: &WorkspaceLayout) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(&layout.output_file)
        .map_err(|_| "renderer did not produce its declared primary output file".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("renderer primary output must be a regular file".into());
    }
    for entry in WalkDir::new(&layout.output_dir).min_depth(1) {
        let entry = entry.map_err(|error| format!("could not inspect renderer output: {error}"))?;
        if entry.path() != layout.output_file {
            return Err(
                "renderer output directory contains files besides the primary output".into(),
            );
        }
    }
    let bytes = fs::read(&layout.output_file)
        .map_err(|error| format!("could not read renderer output: {error}"))?;
    if bytes.is_empty() {
        return Err("renderer returned empty output".into());
    }
    Ok(bytes)
}

pub(crate) fn run_stream(
    cache_root: &Path,
    refresh: bool,
    profile_name: &str,
    profile: &Profile,
    placement: Placement,
    output: Output,
    input: &[u8],
) -> Result<Vec<u8>, String> {
    let cache_key = renderer_cache_key(
        "stream",
        profile_name,
        profile,
        placement,
        output,
        CacheInput::Body(input),
    )?;
    if !refresh {
        if let Some(bytes) = read_cache(cache_root, &cache_key, profile.result())? {
            return validate_renderer_output(profile.result(), output, bytes);
        }
    }
    let result = validate_renderer_output(
        profile.result(),
        output,
        execute_stream(profile, placement, input)?,
    )?;
    write_cache(cache_root, &cache_key, profile.result(), &result)?;
    Ok(result)
}

pub(crate) fn resource_output_path(
    profile: &Profile,
    bytes: &[u8],
) -> Result<crate::output::OutputPath, String> {
    if bytes.is_empty() {
        return Err("renderer returned empty output".into());
    }
    let extension = profile
        .resource_extension()
        .ok_or_else(|| "selected renderer result cannot produce a resource".to_owned())?;
    crate::output::OutputPath::try_new(PathBuf::from(format!(
        "heine-renderers/sha256-{}.{}",
        hex_digest(&Sha256::digest(bytes)),
        extension
    )))
}

fn execute_stream(
    profile: &Profile,
    placement: Placement,
    input: &[u8],
) -> Result<Vec<u8>, String> {
    if profile.protocol != Protocol::Stream {
        return Err("selected profile does not use the stream protocol".into());
    }
    let arguments = match placement {
        Placement::Inline => profile.inline.as_ref(),
        Placement::Block => profile.block.as_ref(),
    }
    .ok_or_else(|| "selected profile does not support this placement".to_owned())?;
    let mut child = Command::new(&profile.program)
        .args(&arguments.arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start renderer: {error}"))?;
    let mut stdout = child.stdout.take().expect("piped renderer stdout");
    let mut stderr = child.stderr.take().expect("piped renderer stderr");
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    child
        .stdin
        .take()
        .expect("piped renderer stdin")
        .write_all(input)
        .map_err(|error| format!("could not write renderer input: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(profile.timeout_seconds);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not wait for renderer: {error}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "renderer exceeded its {}-second timeout",
                profile.timeout_seconds
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "renderer stdout reader panicked".to_owned())
        .and_then(|result| {
            result.map_err(|error| format!("could not read renderer output: {error}"))
        })?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "renderer stderr reader panicked".to_owned())
        .and_then(|result| {
            result.map_err(|error| format!("could not read renderer diagnostics: {error}"))
        })?;
    if status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "renderer exited with {}; {}",
            status,
            String::from_utf8_lossy(&stderr[..stderr.len().min(8192)])
        ))
    }
}

enum CacheInput<'a> {
    Body(&'a [u8]),
    Source(&'a Source),
}

fn cache_input_for_workspace<'a>(input: &'a WorkspaceInput<'a>) -> Result<CacheInput<'a>, String> {
    Ok(match input {
        WorkspaceInput::Body(bytes) => CacheInput::Body(bytes),
        WorkspaceInput::Source(source) => CacheInput::Source(source),
    })
}

fn renderer_cache_key(
    protocol: &str,
    profile_name: &str,
    profile: &Profile,
    placement: Placement,
    output: Output,
    input: CacheInput<'_>,
) -> Result<String, String> {
    let arguments = match placement {
        Placement::Inline => profile.inline.as_ref(),
        Placement::Block => profile.block.as_ref(),
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
            Placement::Inline => "inline",
            Placement::Block => "block",
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
                let bytes = fs::read(path)
                    .map_err(|error| format!("could not read renderer source: {error}"))?;
                hasher.update(bytes);
                hasher.update([0]);
            }
        }
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn read_cache(
    root: &Path,
    key: &str,
    result: &ResultCapabilities,
) -> Result<Option<Vec<u8>>, String> {
    cleanup_cache(root)?;
    let bytes_path = root.join(format!("{key}.bin"));
    let digest_path = root.join(format!("{key}.sha256"));
    let (bytes, digest) = match (fs::read(&bytes_path), fs::read_to_string(&digest_path)) {
        (Ok(bytes), Ok(digest)) => (bytes, digest),
        (Err(error), _) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        (_, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        (Err(error), _) => return Err(format!("could not read renderer cache: {error}")),
        (_, Err(error)) => return Err(format!("could not read renderer cache: {error}")),
    };
    let expected = format!(
        "v1 {} {}",
        cache_result_kind(result),
        hex_digest(&Sha256::digest(&bytes))
    );
    if digest.trim() != expected {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn write_cache(
    root: &Path,
    key: &str,
    result: &ResultCapabilities,
    bytes: &[u8],
) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("could not create renderer cache: {error}"))?;
    cleanup_cache(root)?;
    let digest = format!(
        "v1 {} {}\n",
        cache_result_kind(result),
        hex_digest(&Sha256::digest(bytes))
    );
    write_cache_file(&root.join(format!("{key}.bin")), bytes)?;
    write_cache_file(&root.join(format!("{key}.sha256")), digest.as_bytes())
}

/// Removes incomplete cache artifacts left by an interrupted atomic write.
/// Complete entries are immutable and remain available until the site owner
/// removes `.heine/` or a later cache policy replaces them.
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

fn hex_digest(bytes: &[u8]) -> String {
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

pub(crate) fn validate_embedded(
    result: &ResultCapabilities,
    bytes: Vec<u8>,
) -> Result<String, String> {
    match result {
        ResultCapabilities::Html => String::from_utf8(bytes)
            .map_err(|_| "HTML renderer returned non-UTF-8 embedded output".to_owned()),
        ResultCapabilities::Mathml => validate_xml_fragment(bytes, b"math", "MathML"),
        ResultCapabilities::Svg { .. } => validate_xml_fragment(bytes, b"svg", "SVG"),
        ResultCapabilities::Image { .. } | ResultCapabilities::Link { .. } => {
            Err("selected renderer result cannot be embedded".to_owned())
        }
    }
    .and_then(|result| {
        (!result.trim().is_empty())
            .then_some(result)
            .ok_or_else(|| "renderer returned empty embedded output".to_owned())
    })
}

/// Checks the result forms that are published unchanged as generated files.
pub(crate) fn validate_resource(
    result: &ResultCapabilities,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("renderer returned empty output".to_owned());
    }
    match result {
        ResultCapabilities::Svg { .. } => {
            validate_xml_fragment(bytes.clone(), b"svg", "SVG")?;
            Ok(bytes)
        }
        ResultCapabilities::Image { .. } | ResultCapabilities::Link { .. } => Ok(bytes),
        ResultCapabilities::Html | ResultCapabilities::Mathml => {
            Err("selected renderer result cannot produce a resource".to_owned())
        }
    }
}

fn validate_renderer_output(
    result: &ResultCapabilities,
    output: Output,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, String> {
    match output {
        Output::Embedded => validate_embedded(result, bytes).map(String::into_bytes),
        Output::Resource => validate_resource(result, bytes),
    }
}

fn validate_xml_fragment(
    bytes: Vec<u8>,
    expected_root: &[u8],
    kind: &str,
) -> Result<String, String> {
    let mut reader = Reader::from_reader(bytes.as_slice());
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut root_seen = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Decl(_) | Event::DocType(_)) => {}
            Ok(Event::Start(event)) => {
                if !root_seen {
                    if event.name().as_ref() != expected_root {
                        return Err(format!(
                            "renderer returned {kind} with a different document element"
                        ));
                    }
                    root_seen = true;
                } else if depth == 0 {
                    return Err(format!(
                        "renderer returned more than one {kind} document element"
                    ));
                }
                depth += 1;
                writer
                    .write_event(Event::Start(event.into_owned()))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::Empty(event)) => {
                if !root_seen {
                    if event.name().as_ref() != expected_root {
                        return Err(format!(
                            "renderer returned {kind} with a different document element"
                        ));
                    }
                    root_seen = true;
                } else if depth == 0 {
                    return Err(format!(
                        "renderer returned more than one {kind} document element"
                    ));
                }
                writer
                    .write_event(Event::Empty(event.into_owned()))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::End(event)) => {
                if depth == 0 {
                    return Err(format!("renderer returned malformed {kind}"));
                }
                depth -= 1;
                writer
                    .write_event(Event::End(event.into_owned()))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => {
                if root_seen
                    || !matches!(event, Event::Text(ref text) if text.iter().all(|byte| byte.is_ascii_whitespace()))
                {
                    writer
                        .write_event(event.into_owned())
                        .map_err(|error| error.to_string())?;
                }
            }
            Err(error) => return Err(format!("renderer returned malformed {kind}: {error}")),
        }
        buffer.clear();
    }
    if !root_seen || depth != 0 {
        return Err(format!("renderer returned malformed {kind}"));
    }
    String::from_utf8(writer.into_inner())
        .map_err(|_| format!("renderer returned non-UTF-8 {kind}"))
}

#[derive(Debug, Clone)]
struct Variant {
    arguments: Vec<String>,
}

/// The checked form of a page-owned renderer input. Its filesystem inventory
/// remains with the page loader; Markdown only needs this stable capability
/// when it resolves a named request against its selected profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedInput {
    File,
    Bundle,
}

/// One page-owned renderer input after discovery has fixed its complete
/// filesystem boundary. Later execution receives this checked inventory rather
/// than a raw authored path.
#[derive(Debug, Clone)]
pub(crate) struct Source {
    input: NamedInput,
    root: PathBuf,
    entry: PathBuf,
    files: Vec<PathBuf>,
    declaration_span: SourceSpan,
}

impl Source {
    pub(crate) fn file(path: PathBuf, declaration_span: SourceSpan) -> Self {
        Self {
            input: NamedInput::File,
            root: path.parent().map_or_else(PathBuf::new, Path::to_path_buf),
            entry: path.clone(),
            files: vec![path],
            declaration_span,
        }
    }

    pub(crate) fn bundle(
        root: PathBuf,
        entry: PathBuf,
        files: Vec<PathBuf>,
        declaration_span: SourceSpan,
    ) -> Self {
        Self {
            input: NamedInput::Bundle,
            root,
            entry,
            files,
            declaration_span,
        }
    }

    pub(crate) fn input(&self) -> NamedInput {
        self.input
    }

    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub(crate) fn root(&self) -> &PathBuf {
        &self.root
    }

    pub(crate) fn entry(&self) -> &PathBuf {
        &self.entry
    }

    pub(crate) fn declaration_span(&self) -> SourceSpan {
        self.declaration_span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Inline,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Output {
    Embedded,
    Resource,
}

#[derive(Debug, Clone)]
pub(crate) enum ResultCapabilities {
    Html,
    Mathml,
    Svg {
        materializations: BTreeSet<Output>,
        default_output: Output,
    },
    Image {
        extension: String,
    },
    Link {
        extension: String,
    },
}

impl ResultCapabilities {
    pub(crate) fn output(&self, requested: Option<Output>) -> Option<Output> {
        match self {
            Self::Html | Self::Mathml => match requested {
                None | Some(Output::Embedded) => Some(Output::Embedded),
                Some(Output::Resource) => None,
            },
            Self::Svg {
                materializations,
                default_output,
            } => {
                let output = requested.unwrap_or(*default_output);
                materializations.contains(&output).then_some(output)
            }
            Self::Image { .. } | Self::Link { .. } => match requested {
                None | Some(Output::Resource) => Some(Output::Resource),
                Some(Output::Embedded) => None,
            },
        }
    }

    pub(crate) fn needs_alt(&self, output: Output) -> bool {
        matches!(self, Self::Svg { .. } | Self::Image { .. }) && output == Output::Resource
    }

    pub(crate) fn needs_label(&self, output: Output) -> bool {
        matches!(self, Self::Link { .. }) && output == Output::Resource
    }
}

fn parse_inputs(
    name: &str,
    raw: toml::Spanned<Vec<toml::Spanned<Input>>>,
) -> Result<BTreeSet<Input>, ConfigError> {
    let span = raw.span().into();
    let mut inputs = BTreeSet::new();
    for input in raw.into_inner() {
        let input_span = input.span().into();
        if !inputs.insert(input.into_inner()) {
            return Err(ConfigError {
                span: input_span,
                message: format!("renderer profile {name} inputs must not contain duplicates"),
            });
        }
    }
    if inputs.is_empty() {
        return Err(ConfigError {
            span,
            message: format!("renderer profile {name} inputs must not be empty"),
        });
    }
    Ok(inputs)
}

fn validate_input_extension(
    name: &str,
    protocol: Protocol,
    inputs: &BTreeSet<Input>,
    value: Option<toml::Spanned<String>>,
    inputs_span: SourceSpan,
) -> Result<Option<String>, ConfigError> {
    match value {
        Some(value) if protocol == Protocol::Workspace && inputs.contains(&Input::Body) => {
            validate_extension(value, &format!("renderer profile {name} input_extension")).map(Some)
        }
        Some(value) => Err(ConfigError {
            span: value.span().into(),
            message: format!(
                "renderer profile {name} input_extension is allowed only for a workspace profile that accepts \"body\""
            ),
        }),
        None if protocol == Protocol::Workspace && inputs.contains(&Input::Body) => {
            Err(ConfigError {
                span: inputs_span,
                message: format!(
                    "workspace renderer profile {name} accepts \"body\" and must declare input_extension"
                ),
            })
        }
        None => Ok(None),
    }
}

fn validate_variant(
    name: &str,
    placement: &str,
    protocol: Protocol,
    inputs: &BTreeSet<Input>,
    raw: RawVariant,
) -> Result<Variant, ConfigError> {
    let arguments_span = raw.arguments.span().into();
    let mut placeholders = BTreeMap::<&str, usize>::new();
    let mut arguments = Vec::new();
    for argument in raw.arguments.into_inner() {
        let span = argument.span().into();
        let argument = argument.into_inner();
        arguments.push(argument.clone());
        for placeholder in PLACEHOLDERS {
            if argument.contains(placeholder) && argument != placeholder {
                return Err(ConfigError {
                    span,
                    message: format!(
                        "renderer profile {name} {placement} arguments must use {placeholder} as a complete argument"
                    ),
                });
            }
        }
        if let Some(placeholder) = PLACEHOLDERS
            .into_iter()
            .find(|placeholder| *placeholder == argument)
        {
            *placeholders.entry(placeholder).or_default() += 1;
        }
    }
    if protocol == Protocol::Stream {
        if let Some((placeholder, _)) = placeholders.into_iter().next() {
            return Err(ConfigError {
                span: arguments_span,
                message: format!(
                    "stream renderer profile {name} {placement} arguments must not use workspace placeholder {placeholder}"
                ),
            });
        }
        return Ok(Variant { arguments });
    }
    if placeholders
        .get("{output_file}")
        .copied()
        .unwrap_or_default()
        != 1
    {
        return Err(ConfigError {
            span: arguments_span,
            message: format!(
                "workspace renderer profile {name} {placement} arguments must contain {{output_file}} exactly once"
            ),
        });
    }
    for placeholder in ["{input_file}", "{input_dir}", "{work_dir}"] {
        if placeholders.get(placeholder).copied().unwrap_or_default() > 1 {
            return Err(ConfigError {
                span: arguments_span,
                message: format!(
                    "workspace renderer profile {name} {placement} arguments must contain {placeholder} at most once"
                ),
            });
        }
    }
    if placeholders.contains_key("{input_file}")
        && !inputs.contains(&Input::Body)
        && !inputs.contains(&Input::File)
    {
        return Err(ConfigError {
            span: arguments_span,
            message: format!(
                "workspace renderer profile {name} {placement} uses {{input_file}} but accepts neither \"body\" nor \"file\""
            ),
        });
    }
    if placeholders.contains_key("{input_dir}") && !inputs.contains(&Input::Bundle) {
        return Err(ConfigError {
            span: arguments_span,
            message: format!(
                "workspace renderer profile {name} {placement} uses {{input_dir}} but does not accept \"bundle\""
            ),
        });
    }
    Ok(Variant { arguments })
}

const PLACEHOLDERS: [&str; 4] = ["{input_file}", "{input_dir}", "{work_dir}", "{output_file}"];

struct ResultOptions {
    media_type: Option<toml::Spanned<String>>,
    extension: Option<toml::Spanned<String>>,
    materializations: Option<toml::Spanned<Vec<toml::Spanned<SvgMaterialization>>>>,
    default_output: Option<toml::Spanned<SvgMaterialization>>,
}

fn validate_result(
    name: &str,
    raw_result: toml::Spanned<ResultKind>,
    options: ResultOptions,
) -> Result<ResultCapabilities, ConfigError> {
    let result_span = raw_result.span().into();
    match raw_result.into_inner() {
        ResultKind::Html => {
            reject_result_options(name, &options, "html").map(|()| ResultCapabilities::Html)
        }
        ResultKind::Mathml => {
            reject_result_options(name, &options, "mathml").map(|()| ResultCapabilities::Mathml)
        }
        ResultKind::Svg => validate_svg_result(name, result_span, options),
        ResultKind::Image => validate_resource_result(name, result_span, options, true),
        ResultKind::Link => validate_resource_result(name, result_span, options, false),
    }
}

fn reject_result_options(
    name: &str,
    options: &ResultOptions,
    result: &str,
) -> Result<(), ConfigError> {
    if let Some(span) = option_span(options) {
        return Err(ConfigError {
            span,
            message: format!("{result} renderer profile {name} must not declare resource options"),
        });
    }
    Ok(())
}

fn validate_svg_result(
    name: &str,
    result_span: SourceSpan,
    options: ResultOptions,
) -> Result<ResultCapabilities, ConfigError> {
    let fixed_option_span = options
        .media_type
        .as_ref()
        .map(|value| value.span().into())
        .or_else(|| options.extension.as_ref().map(|value| value.span().into()));
    if let Some(span) = fixed_option_span {
        return Err(ConfigError {
            span,
            message: format!(
                "SVG renderer profile {name} has fixed image/svg+xml media type and svg extension"
            ),
        });
    }
    let raw = options.materializations.ok_or_else(|| ConfigError {
        span: result_span,
        message: format!("SVG renderer profile {name} must declare supported materializations"),
    })?;
    let materializations_span = raw.span().into();
    let mut materializations = BTreeSet::new();
    for value in raw.into_inner() {
        let span = value.span().into();
        if !materializations.insert(Output::from(value.into_inner())) {
            return Err(ConfigError {
                span,
                message: format!(
                    "SVG renderer profile {name} materializations must not contain duplicates"
                ),
            });
        }
    }
    if materializations.is_empty() {
        return Err(ConfigError {
            span: materializations_span,
            message: format!("SVG renderer profile {name} materializations must not be empty"),
        });
    }
    let default = options
        .default_output
        .map(|value| (value.span().into(), Output::from(value.into_inner())));
    if materializations.len() == 2 && default.is_none() {
        return Err(ConfigError {
            span: materializations_span,
            message: format!(
                "SVG renderer profile {name} with both materializations must declare default_output"
            ),
        });
    }
    if materializations.len() == 1
        && let Some((span, _)) = default
    {
        return Err(ConfigError {
            span,
            message: format!(
                "SVG renderer profile {name} must omit default_output when it supports one materialization"
            ),
        });
    }
    if let Some((span, default)) = default
        && !materializations.contains(&default)
    {
        return Err(ConfigError {
            span,
            message: format!(
                "SVG renderer profile {name} default_output must be one of its materializations"
            ),
        });
    }
    let default_output = default.map_or_else(
        || {
            *materializations
                .iter()
                .next()
                .expect("non-empty SVG materializations were checked above")
        },
        |(_, output)| output,
    );
    Ok(ResultCapabilities::Svg {
        materializations,
        default_output,
    })
}

fn validate_resource_result(
    name: &str,
    result_span: SourceSpan,
    options: ResultOptions,
    image: bool,
) -> Result<ResultCapabilities, ConfigError> {
    let materialization_span = options
        .materializations
        .as_ref()
        .map(|value| value.span().into())
        .or_else(|| {
            options
                .default_output
                .as_ref()
                .map(|value| value.span().into())
        });
    if let Some(span) = materialization_span {
        return Err(ConfigError {
            span,
            message: format!(
                "resource renderer profile {name} must not declare SVG materialization options"
            ),
        });
    }
    let media_type = options.media_type.ok_or_else(|| ConfigError {
        span: result_span,
        message: format!("resource renderer profile {name} must declare media_type"),
    })?;
    let media_type_span = media_type.span().into();
    let media_type = media_type
        .into_inner()
        .parse::<mime::Mime>()
        .map_err(|_| ConfigError {
            span: media_type_span,
            message: format!(
                "resource renderer profile {name} media_type must be a valid media type"
            ),
        })?;
    if image && media_type.type_() != mime::IMAGE {
        return Err(ConfigError {
            span: media_type_span,
            message: format!("image renderer profile {name} media_type must start with image/"),
        });
    }
    let extension = options.extension.ok_or_else(|| ConfigError {
        span: result_span,
        message: format!("resource renderer profile {name} must declare extension"),
    })?;
    let extension = validate_extension(
        extension,
        &format!("resource renderer profile {name} extension"),
    )?;
    Ok(if image {
        ResultCapabilities::Image { extension }
    } else {
        ResultCapabilities::Link { extension }
    })
}

fn option_span(options: &ResultOptions) -> Option<SourceSpan> {
    options
        .media_type
        .as_ref()
        .map(|value| value.span().into())
        .or_else(|| options.extension.as_ref().map(|value| value.span().into()))
        .or_else(|| {
            options
                .materializations
                .as_ref()
                .map(|value| value.span().into())
        })
        .or_else(|| {
            options
                .default_output
                .as_ref()
                .map(|value| value.span().into())
        })
}

fn validate_nonempty(value: toml::Spanned<String>, field: &str) -> Result<String, ConfigError> {
    let span = value.span().into();
    let value = value.into_inner();
    if value.trim().is_empty() || value.contains('\0') {
        Err(ConfigError {
            span,
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(value)
    }
}

fn validate_extension(value: toml::Spanned<String>, field: &str) -> Result<String, ConfigError> {
    let span = value.span().into();
    let value = value.into_inner();
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        Ok(value)
    } else {
        Err(ConfigError {
            span,
            message: format!("{field} must be an unprefixed ASCII alphanumeric filename extension"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Protocol {
    Stream,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Input {
    Body,
    File,
    Bundle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ResultKind {
    Html,
    Svg,
    Mathml,
    Image,
    Link,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SvgMaterialization {
    Embedded,
    Resource,
}

impl From<SvgMaterialization> for Output {
    fn from(value: SvgMaterialization) -> Self {
        match value {
            SvgMaterialization::Embedded => Self::Embedded,
            SvgMaterialization::Resource => Self::Resource,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProfile {
    protocol: Protocol,
    program: toml::Spanned<String>,
    timeout_seconds: toml::Spanned<u64>,
    cache_identity: toml::Spanned<String>,
    inputs: toml::Spanned<Vec<toml::Spanned<Input>>>,
    result: toml::Spanned<ResultKind>,
    input_extension: Option<toml::Spanned<String>>,
    media_type: Option<toml::Spanned<String>>,
    extension: Option<toml::Spanned<String>>,
    materializations: Option<toml::Spanned<Vec<toml::Spanned<SvgMaterialization>>>>,
    default_output: Option<toml::Spanned<SvgMaterialization>>,
    inline: Option<RawVariant>,
    block: Option<RawVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVariant {
    arguments: toml::Spanned<Vec<toml::Spanned<String>>>,
}

pub(crate) struct ConfigError {
    pub(crate) span: SourceSpan,
    pub(crate) message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_manager_reuses_a_cleaned_slot_for_one_profile() {
        let temporary_directory = tempfile::tempdir().expect("temporary directory");
        let mut manager =
            WorkspaceManager::new_in(temporary_directory.path()).expect("workspace manager");

        let first = manager.prepare("typst").expect("first slot");
        fs::write(first.join("leftover"), "input").expect("workspace content");
        manager.clean("typst").expect("slot cleanup");
        let second = manager.prepare("typst").expect("reused slot");

        assert_eq!(first, second);
        assert!(
            fs::read_dir(&second)
                .expect("clean workspace")
                .next()
                .is_none()
        );
    }

    #[test]
    fn workspace_manager_keeps_one_slot_per_profile() {
        let temporary_directory = tempfile::tempdir().expect("temporary directory");
        let mut manager =
            WorkspaceManager::new_in(temporary_directory.path()).expect("workspace manager");

        let typst = manager.prepare("typst").expect("Typst slot");
        let mermaid = manager.prepare("mermaid").expect("Mermaid slot");

        assert_ne!(typst, mermaid);
        assert_eq!(manager.slots.len(), 2);
    }

    #[test]
    fn workspace_manager_recovers_an_unavailable_slot_before_reusing_it() {
        let temporary_directory = tempfile::tempdir().expect("temporary directory");
        let mut manager =
            WorkspaceManager::new_in(temporary_directory.path()).expect("workspace manager");

        let path = manager.prepare("typst").expect("workspace slot");
        manager.clean("typst").expect("slot cleanup");
        manager
            .slots
            .get_mut("typst")
            .expect("workspace slot")
            .available = false;
        let recovered = manager.prepare("typst").expect("recovered workspace slot");

        assert_eq!(recovered, path);
        assert!(manager.slots["typst"].available);
    }

    #[test]
    fn workspace_manager_removes_only_stale_owned_sessions() {
        let temporary_directory = tempfile::tempdir().expect("temporary directory");
        let stale = temporary_directory
            .path()
            .join("heine-renderer-session-old");
        let unrelated = temporary_directory.path().join("other-tool-session");
        fs::create_dir(&stale).expect("stale workspace");
        fs::write(stale.join("leftover"), "input").expect("stale workspace content");
        fs::create_dir(&unrelated).expect("unrelated directory");

        let manager =
            WorkspaceManager::new_in(temporary_directory.path()).expect("workspace manager");

        assert!(!stale.exists());
        assert!(unrelated.is_dir());
        assert!(manager.session.path().is_dir());
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
