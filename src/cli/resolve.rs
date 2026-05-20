use crate::cli::Cli;
use crate::store::file::Store;
use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
struct ResolveRow {
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "ADDR")]
    addr: String,
}

pub fn handle(cli: &Cli, hosts: &[String], local: bool) {
    if local {
        resolve_local(cli, hosts);
    } else {
        resolve_dns(hosts);
    }
}

fn resolve_dns(hosts: &[String]) {
    let resolver = match hickory_resolver::TokioAsyncResolver::tokio_from_system_conf() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("DNS init error: {}", e);
            std::process::exit(1);
        }
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let mut rows = Vec::new();
    for host in hosts {
        let host_stripped = host.trim_end_matches('.');
        if host_stripped.parse::<std::net::IpAddr>().is_ok() {
            match rt.block_on(
                resolver.reverse_lookup(host_stripped.parse::<std::net::IpAddr>().unwrap()),
            ) {
                Ok(names) => {
                    for name in names {
                        rows.push(ResolveRow {
                            name: name.to_string().trim_end_matches('.').to_string(),
                            addr: host.to_string(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("{}: {}", host, e);
                }
            }
        } else {
            match rt.block_on(resolver.lookup_ip(host_stripped)) {
                Ok(addrs) => {
                    for addr in addrs {
                        rows.push(ResolveRow {
                            name: host.to_string(),
                            addr: addr.to_string(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("{}: {}", host, e);
                }
            }
        }
    }

    if rows.is_empty() {
        return;
    }
    let mut table = Table::new(&rows);
    table.with(Style::rounded());
    println!("{}", table);
}

fn resolve_local(cli: &Cli, hosts: &[String]) {
    let store = Store::new(&cli.hosts_file);
    let entries = match store.load() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let mut rows = Vec::new();
    for host in hosts {
        let host_stripped = host.trim_end_matches('.');
        if host_stripped.parse::<std::net::IpAddr>().is_ok() {
            // IP → find hostnames pointing to this IP
            for entry in &entries {
                if entry.ip == host_stripped && !entry.disabled {
                    for h in &entry.hostnames {
                        rows.push(ResolveRow {
                            name: h.clone(),
                            addr: entry.ip.clone(),
                        });
                    }
                }
            }
        } else {
            // hostname → find IPs
            for entry in &entries {
                if entry.hostnames.contains(&host_stripped.to_string()) && !entry.disabled {
                    rows.push(ResolveRow {
                        name: host.to_string(),
                        addr: entry.ip.clone(),
                    });
                }
            }
        }
    }

    if rows.is_empty() {
        println!("No local match found.");
        return;
    }
    let mut table = Table::new(&rows);
    table.with(Style::rounded());
    println!("{}", table);
}
