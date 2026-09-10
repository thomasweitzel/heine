use std::path::PathBuf;

use clap::{Parser, Subcommand};
use miette::IntoDiagnostic;

#[derive(Parser, Debug)]
#[command(version, about = "Build and serve a Heine static site")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Only render pages newer than their page definition and content files.
    #[arg(long)]
    quick: bool,

    /// Replace a non-empty output directory that Heine does not yet manage.
    #[arg(long, conflicts_with = "quick")]
    clean_output: bool,

    /// Site root; defaults to the nearest ancestor with heine.toml, content/, and templates/.
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build, serve, and live-reload a site for local development.
    Serve(ServeArgs),
}

#[derive(clap::Args, Debug)]
struct ServeArgs {
    /// Site root; defaults to the nearest ancestor with heine.toml, content/, and templates/.
    #[arg(long)]
    root: Option<PathBuf>,

    /// Replace a non-empty output directory that Heine does not yet manage.
    #[arg(long)]
    clean_output: bool,

    /// Include draft pages in this local preview.
    #[arg(long)]
    drafts: bool,

    /// IP address to bind; defaults to the local machine only.
    #[arg(long, default_value = "127.0.0.1")]
    host: std::net::IpAddr,

    /// First unprivileged port to try; Heine chooses the next free port if needed.
    #[arg(long, default_value_t = 1111, value_parser = port)]
    port: u16,
}

fn port(value: &str) -> Result<u16, String> {
    let port = value.parse::<u16>().map_err(|error| error.to_string())?;
    if port < 1024 {
        return Err("port must be between 1024 and 65535".into());
    }
    Ok(port)
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().into_diagnostic()?;
    let root_argument = match &cli.command {
        Some(Command::Serve(args)) => args.root.clone(),
        None => cli.root,
    };
    let root = match root_argument.or_else(|| heine::find_site_root(&cwd)) {
        Some(root) => root,
        None => {
            return Err(miette::miette!(
                "could not find a site root containing heine.toml, content/, and templates/"
            ));
        }
    };
    if let Some(Command::Serve(args)) = cli.command {
        if cli.quick || cli.clean_output {
            return Err(miette::miette!(
                "--quick and --clean-output apply to builds, not `heine serve`"
            ));
        }
        let options =
            heine::ServeOptions::with_host(args.host, args.port).map_err(miette::Report::new)?;
        let options = if args.clean_output {
            options.clean_output()
        } else {
            options
        };
        let options = if args.drafts {
            options.with_drafts()
        } else {
            options
        };
        return heine::serve_at(&root, options);
    }
    let mode = if cli.quick {
        heine::BuildMode::Quick
    } else {
        heine::BuildMode::Full
    };
    let options = if cli.clean_output {
        heine::BuildOptions::new(mode).clean_output()
    } else {
        heine::BuildOptions::new(mode)
    };
    heine::build_at_with_options(&root, options)
}
