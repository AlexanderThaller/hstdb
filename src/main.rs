#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(rust_2018_idioms, unused_lifetimes, missing_debug_implementations)]
#![forbid(unsafe_code)]

use clap::Parser;

mod client;
mod config;
mod entry;
mod message;
mod opt;
mod run;
mod server;
mod store;

use log::error;
use opt::Opt;

fn main() {
    let opt = Opt::parse();

    match opt.run() {
        Err(run::Error::WriteStdout(io_err)) => {
            // If pipe is closed we can savely ignore that error
            if io_err.kind() == std::io::ErrorKind::BrokenPipe {}
        }

        Err(err) => error!("{}", err),

        Ok(_) => (),
    }
}
