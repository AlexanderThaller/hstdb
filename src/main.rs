#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(rust_2018_idioms, unused_lifetimes, missing_debug_implementations)]
#![forbid(unsafe_code)]

use clap::Parser;
use color_eyre::eyre::Result;

mod client;
mod config;
mod entry;
mod message;
mod opt;
mod run;
mod server;
mod store;

use opt::Opt;

fn main() -> Result<()> {
    color_eyre::install()?;

    let opt = Opt::from_args();

    if let Err(err) = opt.run() {
        let downcast = err.downcast_ref::<run::Error>();

        match downcast {
            Some(run::Error::WriteStdout(ref io_err)) => {
                // If pipe is closed we can savely ignore that error
                if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(());
                }

                Err(err)
            }

            Some(_) | None => Err(err),
        }
    } else {
        Ok(())
    }
}
