mod client;
mod entry;
mod message;
mod opt;
mod server;
mod store;

use anyhow::Result;
use opt::Opt;
use structopt::StructOpt;

fn main() -> Result<()> {
    let opt = Opt::from_args();

    opt.run()?;

    Ok(())
}
