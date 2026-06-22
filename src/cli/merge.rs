use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub fn handle(cli: &crate::cli::Cli, srcs: &[String], target: Option<&PathBuf>) -> bool {
    let target_path = target.cloned().unwrap_or_else(|| cli.hosts_file.clone());
    let date = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    let mut merged = Vec::new();

    for src in srcs {
        let content = match fetch(src) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading '{}': {}", src, e);
                return false;
            }
        };
        let label = src_label(src);
        merged.push(format!(
            "### source: {} — {}\n{}",
            label,
            date,
            strip_comments(&content)
        ));
    }

    // Validate target path to prevent path traversal
    if let Err(e) = crate::util::validation::validate_secure_path(&target_path) {
        eprintln!("Error: {}", e);
        return false;
    }

    let existing = fs::read_to_string(&target_path).unwrap_or_default();
    let preserved = remove_previous_merges(&existing);

    let mut output = preserved.trim_end().to_string();
    output.push_str("\n\n");
    output.push_str(&merged.join("\n\n"));

    // Use random temp file to prevent TOCTOU
    let dir = target_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = match tempfile::NamedTempFile::new_in(dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {}", e);
            return false;
        }
    };
    // Preserve original file permissions to avoid locking out regular users
    let orig_perms = fs::metadata(&target_path)
        .ok()
        .map(|m| m.permissions().mode());
    let mode = orig_perms.unwrap_or(0o100644);
    if let Err(e) = tmp
        .as_file()
        .set_permissions(fs::Permissions::from_mode(mode & 0o777))
    {
        eprintln!("Warning: failed to set permissions: {}", e);
    }
    if tmp.write_all(output.as_bytes()).is_err() {
        return false;
    }
    let _ = tmp.flush();
    let _ = tmp.as_file().sync_all();
    if tmp.persist(&target_path).is_err() {
        eprintln!("Error saving {}", target_path.display());
        return false;
    }

    if !cli.quiet {
        println!(
            "Merged {} source(s) → {}",
            srcs.len(),
            target_path.display()
        );
    }
    true
}

fn fetch(src: &str) -> io::Result<String> {
    if src.starts_with("http://") || src.starts_with("https://") {
        // Block requests to private/reserved IP ranges to prevent SSRF
        validate_url_target(src)?;

        use std::time::Duration;
        let config = ureq::config::Config::builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(30)))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let response = agent
            .get(src)
            .call()
            .map_err(|e| io::Error::other(format!("HTTP request failed: {}", e)))?;
        let mut body = response.into_body();
        body.read_to_string()
            .map_err(|e| io::Error::other(format!("Read response body failed: {}", e)))
    } else {
        fs::read_to_string(src)
    }
}

fn validate_url_target(url_str: &str) -> io::Result<()> {
    use std::net::{IpAddr, ToSocketAddrs};

    // Parse the URL to extract host
    let host = url_str
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");

    if host.is_empty() {
        return Err(io::Error::other("Empty host in URL"));
    }

    // Resolve to IP addresses
    let addrs = (host, 0)
        .to_socket_addrs()
        .map_err(|e| io::Error::other(format!("DNS resolution failed: {}", e)))?;

    for addr in addrs {
        match addr.ip() {
            IpAddr::V4(v4) => {
                if v4.is_private()
                    || v4.is_loopback()
                    || v4.is_link_local()
                    || v4.is_multicast()
                    || v4.is_broadcast()
                    || v4.octets()[0] == 0
                {
                    return Err(io::Error::other(format!(
                        "Blocked request to private/reserved IP: {}",
                        addr.ip()
                    )));
                }
            }
            IpAddr::V6(v6) => {
                if v6.is_loopback()
                    || v6.is_multicast()
                    || v6.is_unspecified()
                    || v6.octets()[0] == 0xfe && v6.octets()[1] >= 0x80
                {
                    return Err(io::Error::other(format!(
                        "Blocked request to private/reserved IP: {}",
                        addr.ip()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn src_label(src: &str) -> String {
    if src.starts_with("http://") || src.starts_with("https://") {
        src.to_string()
    } else {
        std::path::Path::new(src)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

fn strip_comments(content: &str) -> String {
    content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .filter(|l| {
            // Only keep lines that look like valid hosts entries
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.is_empty() {
                return false;
            }
            // First token should be a valid IP, or the line is dropped
            crate::util::validation::is_valid_ip(parts[0])
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn remove_previous_merges(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<&str> = Vec::new();
    let mut skip = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("### source:") {
            skip = true;
            i += 1;
            continue;
        }
        if skip {
            if line.trim().is_empty() || line.starts_with("### source:") {
                skip = false;
                if line.starts_with("### source:") {
                    continue;
                }
            } else {
                i += 1;
                continue;
            }
        }
        result.push(line);
        i += 1;
    }
    result.join("\n")
}
