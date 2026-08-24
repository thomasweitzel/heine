//! The development-server boundary.
//!
//! Heine's build pipeline stays synchronous and filesystem based. This module
//! owns the asynchronous HTTP boundary, while a small coordinator thread runs
//! complete builds and publishes only successful output generations.

mod files;
mod reload;
mod watch;

use std::{
    fs, io,
    net::{IpAddr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    sync::Arc,
};

use miette::Diagnostic;
use thiserror::Error;
use tokio::sync::watch as state_watch;

use crate::{BuildMode, BuildOptions, BuildPublication, build_at_with_publication};

pub(crate) use crate::output::INTERNAL_DIRECTORY;

/// Options for the local development server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServeOptions {
    host: IpAddr,
    port: u16,
    clean_output: bool,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            host: IpAddr::from([127, 0, 0, 1]),
            port: 1111,
            clean_output: false,
        }
    }
}

impl ServeOptions {
    /// Creates options for a loopback server starting at `port`.
    ///
    /// Ports below 1024 are reserved by convention and rejected to prevent
    /// surprising privilege-dependent behavior.
    pub fn new(port: u16) -> Result<Self, ServeOptionError> {
        Self::with_host(IpAddr::from([127, 0, 0, 1]), port)
    }

    /// Creates options for `host`, trying `port` and higher available ports.
    pub fn with_host(host: IpAddr, port: u16) -> Result<Self, ServeOptionError> {
        if port < 1024 {
            return Err(ServeOptionError::PrivilegedPort { port });
        }
        Ok(Self {
            host,
            port,
            clean_output: false,
        })
    }

    pub fn host(&self) -> IpAddr {
        self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Explicitly adopts and replaces an existing unmanaged output directory
    /// for the initial development-server build.
    pub fn clean_output(mut self) -> Self {
        self.clean_output = true;
        self
    }
}

#[derive(Debug, Diagnostic, Error)]
pub enum ServeOptionError {
    #[error("development-server port {port} is below 1024")]
    #[diagnostic(help("choose a port from 1024 through 65535"))]
    PrivilegedPort { port: u16 },
}

#[derive(Debug, Diagnostic, Error)]
enum Error {
    #[error("could not resolve site root {path}")]
    Root {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not bind a development server on {host} at or above port {start_port}")]
    #[diagnostic(help("stop a process using those ports or choose another --port"))]
    NoAvailablePort { host: IpAddr, start_port: u16 },
    #[error("could not bind development server on {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("could not determine the development-server listener address")]
    ListenerAddress {
        #[source]
        source: io::Error,
    },
    #[error("could not configure development-server listener {address}")]
    ConfigureListener {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("could not hand development-server listener {address} to the async runtime")]
    RuntimeListener {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("could not start the development-server runtime")]
    Runtime {
        #[source]
        source: io::Error,
    },
    #[error("development server stopped unexpectedly")]
    Serve {
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PublishedSite {
    pub(crate) base_path: String,
    pub(crate) output_root: PathBuf,
    pub(crate) generation: u64,
}

impl PublishedSite {
    fn from_build(build: BuildPublication, generation: u64) -> Self {
        Self {
            base_path: build.base_path,
            output_root: build.output_root,
            generation,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) published: state_watch::Receiver<PublishedSite>,
}

/// Runs the local development server until the process is interrupted.
pub(crate) fn serve(root: &Path, options: ServeOptions) -> miette::Result<()> {
    let root = fs::canonicalize(root).map_err(|source| {
        miette::Report::new(Error::Root {
            path: root.into(),
            source,
        })
    })?;
    // Register before the first build so a source write in that build's
    // startup window is queued and rebuilt afterwards.
    let watcher = watch::prepare(root.clone()).map_err(miette::Report::new)?;
    let build = if options.clean_output {
        BuildOptions::new(BuildMode::Full).clean_output()
    } else {
        BuildOptions::new(BuildMode::Full)
    };
    let initial = PublishedSite::from_build(build_at_with_publication(&root, build)?, 1);
    let listener = bind_available(options).map_err(miette::Report::new)?;
    let address = listener
        .local_addr()
        .map_err(|source| miette::Report::new(Error::ListenerAddress { source }))?;
    println!("Serving {}", display_url(address, &initial.base_path));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|source| miette::Report::new(Error::Runtime { source }))?;
    runtime.block_on(run(listener, initial, watcher))
}

async fn run(
    listener: TcpListener,
    initial: PublishedSite,
    watcher: watch::PendingWatcher,
) -> miette::Result<()> {
    let address = listener
        .local_addr()
        .map_err(|source| miette::Report::new(Error::ListenerAddress { source }))?;
    listener
        .set_nonblocking(true)
        .map_err(|source| miette::Report::new(Error::ConfigureListener { address, source }))?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|source| miette::Report::new(Error::RuntimeListener { address, source }))?;
    let (published, receiver) = state_watch::channel(initial);
    let _watcher = watcher.start(published).map_err(miette::Report::new)?;
    let state = ServerState {
        published: receiver,
    };
    let app = files::router(Arc::new(state));
    axum::serve(listener, app)
        .await
        .map_err(|source| miette::Report::new(Error::Serve { source }))
}

fn bind_available(options: ServeOptions) -> Result<TcpListener, Error> {
    for port in candidate_ports(options.port()) {
        let address = SocketAddr::new(options.host(), port);
        match TcpListener::bind(address) {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => continue,
            Err(source) => return Err(Error::Bind { address, source }),
        }
    }
    Err(Error::NoAvailablePort {
        host: options.host(),
        start_port: options.port(),
    })
}

fn candidate_ports(start: u16) -> impl Iterator<Item = u16> {
    start..=u16::MAX
}

fn display_url(address: SocketAddr, base_path: &str) -> String {
    format!("http://{address}{base_path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_privileged_port() {
        assert!(matches!(
            ServeOptions::new(1023),
            Err(ServeOptionError::PrivilegedPort { port: 1023 })
        ));
    }

    #[test]
    fn renders_base_path_in_display_url() {
        assert_eq!(
            display_url("127.0.0.1:1111".parse().unwrap(), "/guide/"),
            "http://127.0.0.1:1111/guide/"
        );
    }

    #[test]
    fn tries_ports_in_ascending_order() {
        assert_eq!(
            candidate_ports(1111).take(3).collect::<Vec<_>>(),
            [1111, 1112, 1113]
        );
    }

    #[test]
    fn clean_output_is_opt_in() {
        assert!(!ServeOptions::default().clean_output);
        assert!(ServeOptions::default().clean_output().clean_output);
    }

    #[test]
    fn published_site_uses_the_successful_build_metadata() {
        let published = PublishedSite::from_build(
            BuildPublication {
                base_path: "/guide/".into(),
                output_root: PathBuf::from("public/guide"),
            },
            4,
        );

        assert_eq!(published.base_path, "/guide/");
        assert_eq!(published.output_root, PathBuf::from("public/guide"));
        assert_eq!(published.generation, 4);
    }
}
