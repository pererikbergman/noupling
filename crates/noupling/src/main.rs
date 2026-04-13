mod cli;
mod core;
mod slices;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path } => {
            println!("Scanning: {}", path);
            println!("Not yet implemented.");
        }
        Commands::Audit { snapshot } => {
            match snapshot {
                Some(id) => println!("Auditing snapshot: {}", id),
                None => println!("Auditing latest snapshot..."),
            }
            println!("Not yet implemented.");
        }
        Commands::Report { format } => {
            println!("Generating {} report...", format);
            println!("Not yet implemented.");
        }
    }
}
