#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(rust_2018_idioms, unused_lifetimes, missing_debug_implementations)]
#![forbid(unsafe_code)]

mod client;
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
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();

    let opt = Opt::from_args();

    if let Err(err) = opt.run() {
        error!("{}", err)
    }
}
