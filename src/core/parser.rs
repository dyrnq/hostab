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
            let hostnames: Vec<String> = hosts_str
                .split_whitespace()
                .map(|h| h.to_string())
                .collect();
            let comment = caps.name("comment").map(|m| m.as_str().to_string());

            entry_id += 1;
            entries.push(Entry {
                id: entry_id,
                ip,
                hostnames,
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
        assert_eq!(result[0].hostnames[0], "real.local");
    }
}
