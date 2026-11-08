//! Renderer process execution and isolated workspace management.

use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use walkdir::WalkDir;

use super::{
    Appearance, Output, Placement, Profile, Profiles, Protocol, RendererCache, RendererOutput,
    Source,
    cache::{CacheInput, cache_input_for_workspace, renderer_cache_key},
    validate_renderer_output,
};

pub(crate) enum WorkspaceInput<'a> {
    Body(&'a [u8]),
    Source(&'a Source),
}

pub(crate) struct WorkspaceRequest<'a> {
    pub(crate) runtime: &'a RendererRuntime,
    pub(crate) refresh: bool,
    pub(crate) profile_name: &'a str,
    pub(crate) profile: &'a Profile,
    pub(crate) placement: Placement,
    pub(crate) appearance: Option<Appearance>,
    pub(crate) output: Output,
    pub(crate) input: WorkspaceInput<'a>,
}

/// A failed renderer operation, with optional program-provided diagnostics.
///
/// Process output stays separate from the host's concise failure summary so
/// callers can attach it to the authored request rather than treating it as a
/// fragment of a source label.
#[derive(Debug)]
pub(crate) struct RendererError {
    pub(crate) message: String,
    pub(crate) diagnostics: Option<String>,
}

impl From<String> for RendererError {
    fn from(message: String) -> Self {
        Self {
            message,
            diagnostics: None,
        }
    }
}

impl From<&str> for RendererError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

impl RendererError {
    pub(crate) fn for_appearance(mut self, appearance: Appearance) -> Self {
        self.message = format!(
            "{} picture renderer failed: {}",
            appearance.name(),
            self.message
        );
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RendererRuntime {
    manager: Arc<Mutex<Option<WorkspaceManager>>>,
    cache: RendererCache,
}

impl RendererRuntime {
    pub(crate) fn new(profiles: &Profiles, cache_root: PathBuf) -> Result<Self, String> {
        let manager = profiles
            .uses_workspace()
            .then(WorkspaceManager::new)
            .transpose()?;
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
            cache: RendererCache::open(cache_root)?,
        })
    }
}

const WORKSPACE_SESSION_PREFIX: &str = "heine-renderer-session-";
const STDERR_LIMIT: usize = 8192;

struct RendererStderr {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_renderer_stderr(mut stderr: impl Read) -> std::io::Result<RendererStderr> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let count = match stderr.read(&mut buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(RendererStderr { bytes, truncated });
        }
        let retained = count.min(STDERR_LIMIT - bytes.len());
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained < count;
    }
}

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
        Self::create_in(&std::env::temp_dir())
    }

    #[cfg(test)]
    fn new_in(temporary_directory: &Path) -> Result<Self, String> {
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

pub(crate) fn run_workspace(
    request: WorkspaceRequest<'_>,
) -> Result<RendererOutput, RendererError> {
    let WorkspaceRequest {
        runtime,
        refresh,
        profile_name,
        profile,
        placement,
        appearance,
        output,
        input,
    } = request;
    let cache_key = renderer_cache_key(
        "workspace",
        profile_name,
        profile,
        placement,
        appearance,
        output,
        cache_input_for_workspace(&input),
    )?;
    if !refresh && let Some(result) = runtime.cache.read(&cache_key, profile, output)? {
        return Ok(result);
    }
    let variant = profile.variant(placement, appearance)?;
    let (result, cleanup) = {
        let mut workspace_manager = runtime
            .manager
            .lock()
            .map_err(|_| "renderer workspace runtime is unavailable".to_owned())?;
        if workspace_manager.is_none() {
            *workspace_manager = Some(WorkspaceManager::new()?);
        }
        let manager = workspace_manager
            .as_mut()
            .expect("workspace manager is initialized before use");
        let workspace = manager.prepare(profile_name)?;
        let result = (|| -> Result<RendererOutput, RendererError> {
            let layout = WorkspaceLayout::create(&workspace, profile, input)?;
            let arguments = expand_workspace_arguments(&variant.arguments, &layout);
            execute_workspace_process(profile, &arguments, &layout)?;
            Ok(validate_renderer_output(
                profile,
                output,
                validate_workspace_output(&layout)?,
            )?)
        })();
        let cleanup = manager.clean(profile_name);
        (result, cleanup)
    };
    let result = match (result, cleanup) {
        (Ok(result), Ok(())) => result,
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
    runtime
        .cache
        .write(&cache_key, profile.result(), result.bytes())?;
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
        let output_file = output_dir.join(format!(
            "result.{}",
            profile.result().workspace_output_extension()
        ));
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
) -> Result<(), RendererError> {
    let mut child = Command::new(&profile.program)
        .args(arguments)
        .current_dir(&layout.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start renderer: {error}"))?;
    let stderr = child.stderr.take().expect("piped renderer stderr");
    let stderr_reader = thread::spawn(move || read_renderer_stderr(stderr));
    let status = wait_for_process(&mut child, profile.timeout_seconds)?;
    let stderr = read_process_output(stderr_reader, "diagnostics")?;
    if status.success() {
        Ok(())
    } else {
        Err(renderer_exit_error(status, &stderr))
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

pub(crate) struct StreamRequest<'a> {
    pub(crate) runtime: &'a RendererRuntime,
    pub(crate) refresh: bool,
    pub(crate) profile_name: &'a str,
    pub(crate) profile: &'a Profile,
    pub(crate) placement: Placement,
    pub(crate) appearance: Option<Appearance>,
    pub(crate) output: Output,
    pub(crate) input: &'a [u8],
}

pub(crate) fn run_stream(request: StreamRequest<'_>) -> Result<RendererOutput, RendererError> {
    let StreamRequest {
        runtime,
        refresh,
        profile_name,
        profile,
        placement,
        appearance,
        output,
        input,
    } = request;
    let cache_key = renderer_cache_key(
        "stream",
        profile_name,
        profile,
        placement,
        appearance,
        output,
        CacheInput::Body(input),
    )?;
    if !refresh && let Some(result) = runtime.cache.read(&cache_key, profile, output)? {
        return Ok(result);
    }
    let result = validate_renderer_output(
        profile,
        output,
        execute_stream(profile, placement, appearance, input)?,
    )?;
    runtime
        .cache
        .write(&cache_key, profile.result(), result.bytes())?;
    Ok(result)
}

fn execute_stream(
    profile: &Profile,
    placement: Placement,
    appearance: Option<Appearance>,
    input: &[u8],
) -> Result<Vec<u8>, RendererError> {
    if profile.protocol != Protocol::Stream {
        return Err("selected profile does not use the stream protocol".into());
    }
    let arguments = profile.variant(placement, appearance)?;
    let mut child = Command::new(&profile.program)
        .args(arguments.arguments())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start renderer: {error}"))?;
    let mut stdout = child.stdout.take().expect("piped renderer stdout");
    let stderr = child.stderr.take().expect("piped renderer stderr");
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || read_renderer_stderr(stderr));
    if let Err(error) = child
        .stdin
        .take()
        .expect("piped renderer stdin")
        .write_all(input)
    {
        let cleanup = stop_renderer(&mut child);
        let mut message = format!("could not write renderer input: {error}");
        if let Err(error) = cleanup {
            message.push_str(&format!("; could not terminate and reap renderer: {error}"));
        }
        return Err(message.into());
    }
    let status = wait_for_process(&mut child, profile.timeout_seconds)?;
    let stdout = read_process_output(stdout_reader, "output")?;
    let stderr = read_process_output(stderr_reader, "diagnostics")?;
    if status.success() {
        Ok(stdout)
    } else {
        Err(renderer_exit_error(status, &stderr))
    }
}

fn wait_for_process(
    child: &mut std::process::Child,
    timeout_seconds: u64,
) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not wait for renderer: {error}"))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "renderer exceeded its {timeout_seconds}-second timeout"
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn stop_renderer(child: &mut std::process::Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    if let Err(error) = child.kill() {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        return Err(error);
    }
    child.wait().map(|_| ())
}

fn read_process_output<T>(
    reader: thread::JoinHandle<std::io::Result<T>>,
    kind: &str,
) -> Result<T, String> {
    reader
        .join()
        .map_err(|_| format!("renderer {kind} reader panicked"))
        .and_then(|result| {
            result.map_err(|error| format!("could not read renderer {kind}: {error}"))
        })
}

fn renderer_exit_error(status: std::process::ExitStatus, stderr: &RendererStderr) -> RendererError {
    let mut output = String::from_utf8_lossy(&stderr.bytes).into_owned();
    if stderr.truncated {
        output.push_str("\n[renderer diagnostics truncated]");
    }
    RendererError {
        message: format!("renderer exited with {status}"),
        diagnostics: (!output.trim().is_empty()).then_some(output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_stderr_retains_a_prefix_and_drains_the_stream() {
        let input = vec![b'x'; STDERR_LIMIT * 3];
        let mut reader = std::io::Cursor::new(&input);
        let stderr = read_renderer_stderr(&mut reader).expect("stderr is read");
        assert_eq!(stderr.bytes, vec![b'x'; STDERR_LIMIT]);
        assert!(stderr.truncated);
        assert_eq!(reader.position(), input.len() as u64);

        let exact = read_renderer_stderr(&input[..STDERR_LIMIT]).expect("exact stderr is read");
        assert_eq!(exact.bytes.len(), STDERR_LIMIT);
        assert!(!exact.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn renderer_failure_identifies_truncated_stderr() {
        use std::os::unix::process::ExitStatusExt;

        let stderr = RendererStderr {
            bytes: b"renderer error".to_vec(),
            truncated: true,
        };
        let error = renderer_exit_error(std::process::ExitStatus::from_raw(1 << 8), &stderr);
        let diagnostics = error.diagnostics.expect("renderer diagnostics");
        assert!(diagnostics.contains("renderer error"));
        assert!(diagnostics.contains("diagnostics truncated"));
    }

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
    fn workspace_manager_preserves_another_live_session() {
        let temporary_directory = tempfile::tempdir().expect("temporary directory");
        let first =
            WorkspaceManager::new_in(temporary_directory.path()).expect("first workspace manager");
        let active_session = first.session.path().to_path_buf();
        fs::write(active_session.join("input"), "input").expect("active workspace content");
        let second =
            WorkspaceManager::new_in(temporary_directory.path()).expect("workspace manager");
        assert!(active_session.is_dir());
        assert_eq!(
            fs::read_to_string(active_session.join("input")).expect("active workspace content"),
            "input"
        );
        assert!(second.session.path().is_dir());
        assert_ne!(active_session, second.session.path());
    }
}
