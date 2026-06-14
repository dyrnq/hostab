use super::model::Entry;
use regex::Regex;

/// Parse a hosts file from a string
pub fn parse(content: &str) -> Vec<Entry> {
    let entry_re = Regex::new(
        r"^(?P<disabled>\s*#\s*)?(?P<ip>\S+)\s+(?P<hosts>.+?)(?:\s*#\s*(?P<comment>.*))?$",
    )
    .unwrap();

    let mut entries: Vec<Entry> = Vec::new();
    let mut entry_id = 0usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(caps) = entry_re.captures(line) {
            let is_disabled = caps.name("disabled").is_some();
            let ip = caps.name("ip").unwrap().as_str().to_string();

            // Skip comment lines that look like disabled entries
            if is_disabled && ip.parse::<std::net::IpAddr>().is_err() {
                continue;
            }

            let hosts_str = caps.name("hosts").unwrap().as_str();
            let all_hosts: Vec<String> = hosts_str
                .split_whitespace()
                .map(|h| h.to_string())
                .collect();
            let comment = caps.name("comment").map(|m| m.as_str().to_string());

            let (canonical, aliases) = if let Some((first, rest)) = all_hosts.split_first() {
                (first.clone(), rest.to_vec())
            } else {
                continue;
            };

            entry_id += 1;
            entries.push(Entry {
                id: entry_id,
                ip,
                canonical,
                aliases,
                comment,
                disabled: is_disabled,
                raw: Some(line.to_string()),
            });
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let input = "127.0.0.1 localhost\n::1 localhost\n";
        let result = parse(input);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_with_comment() {
        let input = "10.0.0.1 app.local # my app\n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].comment, Some("my app".to_string()));
    }

    #[test]
    fn test_parse_skips_comments() {
        let input = "# this is a comment\n127.0.0.1 real.local\n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].canonical, "real.local");
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse("").len(), 0);
        assert_eq!(parse("  \n  \n").len(), 0);
    }

    #[test]
    fn test_parse_ipv6_full() {
        let input = "fe80::1 localhost6\n2001:db8::1 example.com\n";
        let result = parse(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].ip, "fe80::1");
    }

    #[test]
    fn test_parse_tab_separated() {
        let input = "127.0.0.1\tlocalhost\n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].canonical, "localhost");
    }

    #[test]
    fn test_parse_disabled_entry() {
        let input = "# 10.0.0.1 blocked.local\n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].disabled);
    }

    #[test]
    fn test_parse_disabled_with_comment() {
        let input = "# 10.0.0.1 blocked.local # blocked reason\n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].disabled);
        assert_eq!(result[0].comment, Some("blocked reason".to_string()));
    }

    #[test]
    fn test_parse_mixed_lines() {
        // # comment is parsed as entry ip="#" host="comment" (parser ambiguity: # can be IP)
        let input = "127.0.0.1 localhost\n# 10.0.0.1 blocked.local\n::1 localhost6\n";
        let result = parse(input);
        assert_eq!(result.len(), 3);
        let disabled: Vec<_> = result.iter().filter(|e| e.disabled).collect();
        let enabled: Vec<_> = result.iter().filter(|e| !e.disabled).collect();
        assert_eq!(disabled.len(), 1);
        assert_eq!(enabled.len(), 2);
        assert_eq!(disabled[0].canonical, "blocked.local");
    }

    #[test]
    fn test_parse_extra_spaces() {
        let input = "  127.0.0.1     localhost   \n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].canonical, "localhost");
    }

    #[test]
    fn test_parse_many_hostnames() {
        let hosts: Vec<String> = (0..10).map(|i| format!("host{}.local", i)).collect();
        let line = format!("10.0.0.1 {}\n", hosts.join(" "));
        let result = parse(&line);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].canonical, hosts[0].clone());
        assert_eq!(result[0].aliases, hosts[1..].to_vec());
    }

    #[test]
    fn test_parse_comment_with_hash_in_content() {
        let input = "10.0.0.1 example.com # version #2\n";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].comment, Some("version #2".to_string()));
    }

    #[test]
    fn test_parse_dos_newlines() {
        let input = "127.0.0.1 localhost\r\n::1 localhost6\r\n";
        let result = parse(input);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_no_trailing_newline() {
        let input = "127.0.0.1 localhost";
        let result = parse(input);
        assert_eq!(result.len(), 1);
    }
}

#[test]
fn test_parse_canonical_split() {
    let input = "10.0.0.1 app.local api.local web.local
";
    let result = parse(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].canonical, "app.local");
    assert_eq!(result[0].aliases, vec!["api.local", "web.local"]);
}

#[test]
fn test_parse_single_no_alias() {
    let input = "10.0.0.1 app.local
";
    let result = parse(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].canonical, "app.local");
    assert!(result[0].aliases.is_empty());
}

#[test]
fn test_parse_canonical_disabled() {
    let input = "# 10.0.0.1 blocked.local alt.local
";
    let result = parse(input);
    assert_eq!(result.len(), 1);
    assert!(result[0].disabled);
    assert_eq!(result[0].canonical, "blocked.local");
    assert_eq!(result[0].aliases, vec!["alt.local"]);
}
