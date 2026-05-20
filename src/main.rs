#![allow(dead_code)]
use clap::Parser;

mod cli;
mod core;
mod output;
mod store;
mod util;

use cli::{Cli, Commands, EntryCommands};

fn main() {
    let cli = Cli::parse();

    if cli.hosts_file.to_string_lossy().contains("etc/hosts") && !util::validation::is_root() {
        match &cli.command {
            Commands::Entry(EntryCommands::List { .. })
            | Commands::Resolve { .. }
            | Commands::Verify { .. }
            | Commands::Merge {
                target: Some(_), ..
            }
            | Commands::Completion { .. }
            | Commands::Version
            | Commands::Cat => {}
            _ => eprintln!(
                "Warning: modifying {} may require root privileges.",
                cli.hosts_file.display()
            ),
        }
    }

    match &cli.command {
        Commands::Entry(cmd) => match cmd {
            EntryCommands::List {
                ipv4,
                ipv6,
                expand,
                pattern,
                ignore_case,
            } => {
                cli::entry::handle_list(
                    &cli,
                    *ipv4,
                    *ipv6,
                    *expand,
                    pattern.as_deref(),
                    *ignore_case,
                );
            }
            EntryCommands::Add { ip, hosts, comment } => {
                cli::entry::handle_add(&cli, ip, hosts, comment.as_deref())
            }
            EntryCommands::Rm { hosts, ip } => cli::entry::handle_rm(&cli, hosts, ip.as_deref()),
            EntryCommands::Disable { hosts, ip } => {
                cli::entry::handle_disable(&cli, hosts, ip.as_deref())
            }
            EntryCommands::Enable { hosts, ip } => {
                cli::entry::handle_enable(&cli, hosts, ip.as_deref())
            }
            EntryCommands::Toggle { host, ip } => {
                cli::entry::handle_toggle(&cli, host, ip.as_deref())
            }
            EntryCommands::Edit { host, ip } => cli::entry::handle_edit(&cli, host, ip),
        },
        Commands::Verify { strict } => cli::verify::handle_verify(&cli, *strict),
        Commands::Merge { src, target } => {
            cli::merge::handle(&cli, src, target.as_ref());
        }
        Commands::Resolve { hosts, local } => cli::resolve::handle(&cli, hosts, *local),
        Commands::Cat => match std::fs::read_to_string(&cli.hosts_file) {
            Ok(c) => print!("{}", c),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Completion { shell } => {
            use clap::CommandFactory;
            use clap_complete::{generate, shells};
            let mut cmd = Cli::command();
            match shell.as_str() {
                "bash" => generate(shells::Bash, &mut cmd, "hostab", &mut std::io::stdout()),
                "zsh" => generate(shells::Zsh, &mut cmd, "hostab", &mut std::io::stdout()),
                "fish" => generate(shells::Fish, &mut cmd, "hostab", &mut std::io::stdout()),
                _ => eprintln!("Unsupported shell: {}. Supported: bash, zsh, fish", shell),
            }
        }
        Commands::Version => {
            println!("hostab {}", env!("CARGO_PKG_VERSION"));
            println!("  commit:  {}", env!("GIT_HASH"));
            println!("  built:   {}", env!("BUILD_DATE"));
            println!("  target:  {}", env!("TARGET_TRIPLE"));
        }
    }
}
