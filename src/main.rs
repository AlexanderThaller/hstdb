#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(rust_2018_idioms, unused_lifetimes, missing_debug_implementations)]
#![forbid(unsafe_code)]

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
use structopt::StructOpt;

fn main() {
    let opt = Opt::from_args();

    if let Err(err) = opt.run() {
        error!("{}", err);
    }
}
