use crate::cli::{print_output, Cli};
use crate::store::file::Store;

pub fn handle_list(cli: &Cli, ipv4: bool, ipv6: bool, pattern: Option<&str>, ignore_case: bool) {
    let store = Store::new(&cli.hosts_file);
    let entries = match store.load() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let re = pattern.and_then(|p| match build_regex(p, ignore_case) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("Warning: invalid filter pattern: {}", e);
            None
        }
    });

    let mut rows = Vec::new();
    for entry in &entries {
        if entry.disabled {
            continue;
        }
        if entry.canonical.is_empty() && entry.aliases.is_empty() {
            continue;
        }
        if let Some(ref re) = re {
            let ok = re.is_match(&entry.ip)
                || entry.aliases.iter().any(|h| re.is_match(h))
                || re.is_match(&entry.canonical)
                || entry.comment.as_ref().is_some_and(|c| re.is_match(c));
            if !ok {
                continue;
            }
        }
        if ipv4 && entry.ip.parse::<std::net::Ipv4Addr>().is_err() {
            continue;
        }
        if ipv6 && entry.ip.parse::<std::net::Ipv6Addr>().is_err() {
            continue;
        }
        let host_str = if entry.aliases.is_empty() {
            entry.canonical.clone()
        } else {
            let mut parts = vec![entry.canonical.clone()];
            parts.extend(entry.aliases.clone());
            parts.join(" ")
        };
        rows.push(crate::core::model::Row {
            ip: entry.ip.clone(),
            host: host_str,
            comment: entry.comment.clone(),
            canonical: Some(entry.canonical.clone()),
            aliases: entry.aliases.clone(),
        });
    }
    print_output(cli, &rows);
}

pub fn handle_add(
    cli: &Cli,
    ip: &str,
    hosts: &[String],
    canonical: Option<&str>,
    aliases: &[String],
    comment: Option<&str>,
) {
    let canon: Vec<String> = if let Some(c) = canonical {
        // Explicit canonical, hosts are extra positionals
        let mut all = vec![c.to_string()];
        all.extend(hosts.iter().cloned());
        all.extend(aliases.iter().cloned());
        all
    } else if !hosts.is_empty() {
        // Positional: first is canonical, rest are aliases
        let mut all = hosts.to_vec();
        all.extend(aliases.iter().cloned());
        all
    } else {
        aliases.to_vec()
    };

    let store = Store::new(&cli.hosts_file);
    match store.add_entry(ip, &canon, comment) {
        Ok(duplicates) => {
            if !cli.quiet {
                println!("Added {} {}", ip, canon.join(" "));
            }
            for d in &duplicates {
                eprintln!("Warning: '{}' already exists on another IP", d);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_rm(cli: &Cli, hosts: &[String], ip: Option<&str>) {
    let store = Store::new(&cli.hosts_file);
    if let Some(ip) = ip {
        match store.remove_by_ip(ip) {
            Ok(count) => {
                if !cli.quiet {
                    println!("Removed {} entry(s) for IP {}", count, ip);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match store.remove_hostnames(hosts) {
            Ok(count) => {
                if !cli.quiet {
                    println!("Removed {} hostname(s)", count);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

pub fn handle_disable(cli: &Cli, hosts: &[String], ip: Option<&str>) {
    let store = Store::new(&cli.hosts_file);
    if let Some(ip) = ip {
        match store.disable_by_ip(ip) {
            Ok(n) => {
                if !cli.quiet {
                    println!("Disabled {} entry(s) for IP {}", n, ip);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match store.disable_hostname(hosts) {
            Ok(n) => {
                if !cli.quiet {
                    println!("Disabled {} hostname(s)", n);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

pub fn handle_enable(cli: &Cli, hosts: &[String], ip: Option<&str>) {
    let store = Store::new(&cli.hosts_file);
    if let Some(ip) = ip {
        match store.enable_by_ip(ip) {
            Ok(n) => {
                if !cli.quiet {
                    println!("Enabled {} entry(s) for IP {}", n, ip);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match store.enable_hostname(hosts) {
            Ok(n) => {
                if !cli.quiet {
                    println!("Enabled {} hostname(s)", n);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

pub fn handle_toggle(cli: &Cli, host: &str, ip: Option<&str>) {
    let store = Store::new(&cli.hosts_file);
    if let Some(ip) = ip {
        match store.toggle_by_ip(ip) {
            Ok(msg) => {
                if !cli.quiet {
                    println!("{}", msg);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match store.toggle_hostname(host) {
            Ok(msg) => {
                if !cli.quiet {
                    println!("{}", msg);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

pub fn handle_edit(cli: &Cli, host: &str, ip: &str) {
    let store = Store::new(&cli.hosts_file);
    match store.move_hostname(host, ip) {
        Ok(0) => eprintln!("Hostname '{}' not found", host),
        Ok(_) => {
            if !cli.quiet {
                println!("Moved '{}' to {}", host, ip);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Build regex from pattern. `*`/`?` → glob, else literal substring.
pub(crate) fn build_regex(pattern: &str, ignore_case: bool) -> Result<regex::Regex, String> {
    if pattern.is_empty() {
        return Err("empty pattern".into());
    }
    // Limit pattern length to prevent resource exhaustion
    if pattern.len() > 1024 {
        return Err("pattern exceeds maximum length of 1024 characters".into());
    }
    let has_wildcards = pattern.contains('*') || pattern.contains('?');
    let re_str = if has_wildcards {
        glob_to_regex(pattern)
    } else {
        regex::escape(pattern)
    };
    let mut b = regex::RegexBuilder::new(&re_str);
    if ignore_case {
        b.case_insensitive(true);
    }
    b.build()
        .map_err(|e| format!("invalid regex pattern: {}", e))
}

fn glob_to_regex(pattern: &str) -> String {
    let mut out = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('$');
    out
}
