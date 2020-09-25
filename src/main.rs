mod client;
mod entry;
mod message;
mod opt;
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
