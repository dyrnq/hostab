pub mod entry;
pub mod merge;
pub mod resolve;
pub mod serve;
pub mod verify;

use crate::core::model::Row;
use std::collections::HashMap;

use clap::{Parser, Subcommand, ValueHint};
use std::path::PathBuf;

use crate::store::file::DEFAULT_HOSTS_PATH;

/// Your dev tool to manage /etc/hosts like a pro
#[derive(Parser, Debug)]
#[command(name = "hostab", version, about, long_about = None)]
pub struct Cli {
    /// Path to the hosts file
    #[arg(long, env = "HOSTS_FILE", default_value = DEFAULT_HOSTS_PATH, value_hint = ValueHint::FilePath, global = true)]
    pub hosts_file: PathBuf,

    /// Suppress output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Output format: table, raw, markdown, json
    #[arg(short = 'o', long, default_value = "table", global = true)]
    pub out: String,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage individual hosts entries
    #[command(subcommand, alias = "e")]
    Entry(EntryCommands),

    /// Validate the hosts file
    Verify {
        #[arg(long)]
        strict: bool,
    },

    /// Print raw hosts file content
    Cat,

    /// Resolve hostnames (DNS or local hosts file)
    Resolve {
        /// Hostnames or IPs to resolve
        hosts: Vec<String>,

        /// Only check the local hosts file, skip DNS
        #[arg(short, long)]
        local: bool,
    },

    /// Merge hosts entries from multiple sources
    Merge {
        /// Source files or URLs (repeatable)
        #[arg(short, long, required = true, value_hint = ValueHint::FilePath)]
        src: Vec<String>,

        /// Target hosts file (default: system hosts)
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        target: Option<PathBuf>,
    },

    /// Generate shell completion scripts
    Completion { shell: String },

    /// Show detailed version info
    Version,

    /// Start HTTP REST API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3456")]
        port: u16,
    },
}

// ── Entry subcommands ──────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum EntryCommands {
    /// List all entries
    List {
        /// Show only IPv4 entries
        #[arg(long)]
        ipv4: bool,

        /// Show only IPv6 entries
        #[arg(long, conflicts_with = "ipv4")]
        ipv6: bool,

        /// One row per hostname (default: compact by IP)
        #[arg(long)]
        expand: bool,

        /// Filter entries matching pattern (supports * and ? wildcards)
        #[arg(short = 'f', long = "filter")]
        pattern: Option<String>,

        /// Case insensitive match
        #[arg(short = 'i', long)]
        ignore_case: bool,
    },

    /// Add a new entry
    Add {
        /// IP address
        ip: String,

        /// Hostnames
        hosts: Vec<String>,

        /// Optional comment
        #[arg(long)]
        comment: Option<String>,
    },

    /// Remove entries
    Rm {
        /// Hostnames to remove
        hosts: Vec<String>,

        /// Remove all entries for this IP
        #[arg(long)]
        ip: Option<String>,
    },

    /// Disable hostnames (comment out, split from shared IP)
    Disable {
        /// Hostnames to disable
        hosts: Vec<String>,

        /// Disable all entries for this IP
        #[arg(long)]
        ip: Option<String>,
    },

    /// Enable hostnames (uncomment, merge back to IP)
    Enable {
        /// Hostnames to enable
        hosts: Vec<String>,

        /// Enable all disabled entries for this IP
        #[arg(long)]
        ip: Option<String>,
    },

    /// Toggle a hostname enabled/disabled
    Toggle {
        /// Hostname to toggle
        host: String,

        /// Toggle all entries for this IP
        #[arg(long)]
        ip: Option<String>,
    },

    /// Move a hostname to a new IP
    Edit {
        /// Hostname to move
        host: String,

        /// New IP address
        #[arg(long)]
        ip: String,
    },
}

// ── Shared output utilities ─────────────────────────────────────

/// Merge rows with the same IP into one row, joining hostnames with spaces.
pub fn compact_rows(rows: &[Row]) -> Vec<Row> {
    let mut map: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for row in rows {
        let key = row.ip.clone();
        let (hosts, comments) = map.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            (Vec::new(), Vec::new())
        });
        hosts.push(row.host.clone());
        if let Some(ref c) = row.comment {
            if !c.is_empty() && !comments.contains(c) {
                comments.push(c.clone());
            }
        }
    }

    order
        .into_iter()
        .filter_map(|ip| {
            map.remove(&ip).map(|(hosts, comments)| Row {
                ip,
                host: hosts.join(" "),
                comment: if comments.is_empty() {
                    None
                } else {
                    Some(comments.join("; "))
                },
            })
        })
        .collect()
}

/// Print rows in the requested output format
pub fn print_output(cli: &Cli, rows: &[Row]) {
    if cli.quiet {
        return;
    }

    let output_str = match cli.out.as_str() {
        "json" => crate::output::json::format_json_rows(rows),
        "raw" => crate::output::table::format_raw(rows),
        "markdown" => crate::output::table::format_markdown(rows),
        _ => crate::output::table::format_table(rows),
    };

    println!("{}", output_str);
}
