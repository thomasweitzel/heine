use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// An isolated temporary directory removed when its test finishes.
///
/// A process-local sequence chooses concise names; retrying on an existing
/// directory also makes concurrent test binaries and stale directories safe.
pub(crate) struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    pub(crate) fn new() -> Self {
        let temporary_root = std::env::temp_dir();

        for _ in 0..1_000 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = temporary_root.join(format!("h-{sequence}"));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create temporary test directory: {error}"),
            }
        }

        panic!("could not allocate a unique temporary test directory");
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Deref for TestDirectory {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        // Do not hide an assertion failure when best-effort test cleanup fails.
        let _ = fs::remove_dir_all(&self.path);
    }
}
