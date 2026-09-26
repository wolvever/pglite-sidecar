//! Sidecar that embeds PostgreSQL via [`pglite`] and exposes a Unix-socket URI
//! so unmodified Postgres clients (Go `pgx`, SQLx, `psql`, …) can connect.

#[cfg(not(unix))]
compile_error!(
    "pglite-sidecar supports Linux and macOS only; the pglite-rs socket gateway is unavailable on Windows"
);

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use pglite::{MultiProcessOptions, PGlite};

/// Embed PostgreSQL via pglite-rs and serve it on a Unix socket.
#[derive(Parser, Debug)]
#[command(
    name = "pglite-sidecar",
    version,
    about = "Embed PostgreSQL via pglite-rs and expose it over a Unix socket"
)]
struct Args {
    /// Directory for the PostgreSQL data files (created if missing)
    #[arg(
        short = 'd',
        long = "data-dir",
        default_value = "./data",
        value_name = "DIR"
    )]
    data_dir: PathBuf,

    /// Extra postmaster slots reserved for external clients (ORM / driver pools)
    #[arg(long = "extra-connections", default_value_t = 4, value_name = "N")]
    extra_connections: usize,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pglite-sidecar: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.extra_connections == 0 {
        return Err("--extra-connections must be at least 1".into());
    }

    eprintln!(
        "opening pglite (multi-process) data dir {} …",
        args.data_dir.display()
    );

    let mut options = MultiProcessOptions::default();
    options.extra_connections = args.extra_connections;

    let db = PGlite::open_multi_process(&args.data_dir, options).await?;
    // First unix_uri() returns the child postmaster's native socket (or starts
    // the in-process gateway). Keep `db` alive for the process lifetime.
    let uri = db.unix_uri().await?;

    {
        let mut stdout = io::stdout();
        writeln!(stdout, "{uri}")?;
        stdout.flush()?;
    }

    eprintln!("ready — connection URI is on stdout (first line)");
    eprintln!("waiting for SIGINT / SIGTERM (Ctrl-C to stop)");

    wait_for_shutdown().await?;

    eprintln!("shutting down…");
    db.close().await?;
    eprintln!("closed");
    Ok(())
}

async fn wait_for_shutdown() -> io::Result<()> {
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => {}
        _ = sigterm.recv() => {}
    }
    Ok(())
}
