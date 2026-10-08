use clap::Parser;
use std::{
    io::{self},
    sync::Arc,
};

use packettap::{
    config::{Args, Config},
    proxy::run,
};

fn main() -> io::Result<()> {
    let args = Args::parse();

    let config = Arc::new(Config::from(args.clone()));

    run(args, config)
}
