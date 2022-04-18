#![allow(clippy::try_err)]

use std::error::Error;
use std::fs::File;
use std::io::Write;

use clap::Parser;

pub mod ctf;
mod report;
pub mod rsgen;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    filename: String,

    #[clap(long, short)]
    report: bool,

    #[clap(long, short)]
    debug: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let ctf_info = ctf::Ctf::from_file(&args.filename)?;

    let code = rsgen::emit(&ctf_info);
    let mut file = File::create(format!("{}.rs", ctf_info.libname))?;
    file.write_all(code.as_bytes())?;

    if args.report {
        report::print(&ctf_info, args.debug);
    }

    Ok(())
}
