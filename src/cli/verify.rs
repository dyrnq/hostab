use crate::cli::Cli;
use crate::store::file::Store;
use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
struct IssueRow {
    #[tabled(rename = "LINE")]
    line: usize,
    ip: String,
    host: String,
    issue: String,
}

pub fn handle_verify(cli: &Cli, strict: bool) {
    let store = Store::new(&cli.hosts_file);
    match store.verify() {
        Ok(issues) => {
            if issues.is_empty() {
                if !cli.quiet { println!("No issues found."); }
            } else {
                let rows: Vec<IssueRow> = issues.iter().map(|(ln, ip, host, issue)| IssueRow {
                    line: *ln, ip: ip.clone(), host: host.clone(), issue: issue.clone(),
                }).collect();
                let mut table = Table::new(&rows);
                table.with(Style::rounded());
                println!("{}", table);
                if !cli.quiet {
                    println!("Found {} issue(s)", issues.len());
                }
                if strict { std::process::exit(1); }
            }
        }
        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
    }
}
