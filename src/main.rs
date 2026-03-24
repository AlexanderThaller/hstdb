//! Binary entry point for the `hstdb` command-line application.

use clap::Parser;
use color_eyre::{
    Report,
    eyre::Context,
};

mod client;
mod config;
mod entry;
mod message;
mod opt;
mod run;
mod server;
mod store;
mod version;

use opt::Opt;

fn main() -> color_eyre::Result<()> {
    color_eyre::install().context("Failed to install color_eyre")?;

    let opt = Opt::parse();

    match opt.run() {
        Err(err) if is_broken_pipe(&err) => Ok(()),
        result => result,
    }
}

fn is_broken_pipe(err: &Report) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::BrokenPipe)
    })
}
