use regex::Regex;
use std::path::Path;

/// Validate that an IP address is well-formed
pub fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>().is_ok()
}

/// Validate IPv4
#[allow(dead_code)]
pub fn is_valid_ipv4(ip: &str) -> bool {
    ip.parse::<std::net::Ipv4Addr>().is_ok()
}

/// Validate IPv6
#[allow(dead_code)]
pub fn is_valid_ipv6(ip: &str) -> bool {
    ip.parse::<std::net::Ipv6Addr>().is_ok()
}

/// Validate a hostname according to RFC 1123
pub fn is_valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }

    let hostname_re =
        Regex::new(r"^(?i)([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)*[a-z0-9]([a-z0-9-]*[a-z0-9])?$")
            .unwrap();

    hostname_re.is_match(hostname)
}

/// Validate a comment string (no newlines or control characters)
pub fn is_valid_comment(comment: &str) -> bool {
    !comment.contains('\n')
        && !comment.contains('\r')
        && comment.chars().all(|c| !c.is_control() || c == '\t')
}

/// Normalize an IP address to its canonical form
#[allow(dead_code)]
pub fn normalize_ip(ip: &str) -> Option<String> {
    match ip.parse::<std::net::IpAddr>() {
        Ok(addr) => Some(addr.to_string()),
        Err(_) => None,
    }
}

/// Normalize a hostname (lowercase, trim)
#[allow(dead_code)]
pub fn normalize_hostname(hostname: &str) -> String {
    hostname.trim().to_lowercase()
}

/// Validate a path for security: no path traversal, no null bytes
pub fn validate_secure_path(path: &Path) -> Result<(), std::io::Error> {
    let path_str = path.to_string_lossy();

    // Check for null bytes
    if path_str.contains('\0') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path contains null bytes",
        ));
    }

    // Check for path traversal
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path traversal detected: {}", path.display()),
            ));
        }
    }

    // Check for encoded traversal patterns
    let lower = path_str.to_lowercase();
    if lower.contains("%2e%2e") || lower.contains("..%2f") || lower.contains("%2e%2e%2f") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Encoded path traversal detected: {}", path.display()),
        ));
    }

    Ok(())
}

/// Check if running as root (Unix only; on Windows always returns false)
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ip() {
        assert!(is_valid_ip("127.0.0.1"));
        assert!(is_valid_ip("::1"));
        assert!(is_valid_ip("10.0.0.1"));
        assert!(!is_valid_ip("not.an.ip"));
        assert!(!is_valid_ip("999.999.999.999"));
    }

    #[test]
    fn test_valid_hostname() {
        assert!(is_valid_hostname("localhost"));
        assert!(is_valid_hostname("api.myapp.local"));
        assert!(is_valid_hostname("a.io"));
        assert!(!is_valid_hostname(""));
        assert!(!is_valid_hostname("-bad.com"));
        assert!(!is_valid_hostname("bad-.com"));
    }

    #[test]
    fn test_normalize_ip() {
        assert_eq!(normalize_ip("127.0.0.1"), Some("127.0.0.1".to_string()));
        assert_eq!(normalize_ip("::1"), Some("::1".to_string()));
        assert_eq!(normalize_ip("invalid"), None);
    }

    #[test]
    fn test_validate_secure_path() {
        assert!(validate_secure_path(Path::new("/etc/hosts")).is_ok());
        assert!(validate_secure_path(Path::new("/etc/../hosts")).is_err());
        assert!(validate_secure_path(Path::new("/etc/\0hosts")).is_err());
    }

    #[test]
    fn test_valid_ipv6_compressed() {
        assert!(is_valid_ip("::"));
        assert!(is_valid_ip("::1"));
        assert!(is_valid_ip("fe80::1"));
        assert!(is_valid_ip("2001:db8::1"));
    }

    #[test]
    fn test_valid_ip_boundary() {
        assert!(!is_valid_ip(""));
        assert!(!is_valid_ip("256.0.0.1"));
        assert!(is_valid_ip("0.0.0.0"));
        assert!(is_valid_ip("255.255.255.255"));
    }

    #[test]
    fn test_valid_hostname_long() {
        // 63.63.63.61 = exactly 253 chars (RFC 1035 max)
        let long =
            "a".repeat(63) + "." + &"b".repeat(63) + "." + &"c".repeat(63) + "." + &"d".repeat(61);
        assert_eq!(long.len(), 253);
        assert!(is_valid_hostname(&long));

        let too_long = long.clone() + "e";
        assert!(!is_valid_hostname(&too_long));
    }

    #[test]
    fn test_valid_hostname_single_label() {
        assert!(is_valid_hostname("a"));
        assert!(!is_valid_hostname("-start"));
        assert!(!is_valid_hostname("end-"));
        assert!(is_valid_hostname("a-b"));
    }

    #[test]
    fn test_valid_comment() {
        assert!(is_valid_comment("normal comment"));
        assert!(!is_valid_comment("line1\nline2"));
        assert!(!is_valid_comment("has\r\ncrlf"));
        assert!(is_valid_comment("has\ttab"));
        assert!(!is_valid_comment("\x00null"));
    }

    #[test]
    fn test_normalize_hostname() {
        assert_eq!(normalize_hostname("  LOCALHOST  "), "localhost");
        assert_eq!(normalize_hostname("Example.COM"), "example.com");
        assert_eq!(normalize_hostname(""), "");
    }
}
