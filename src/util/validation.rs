use regex::Regex;
use std::path::Path;

/// Validate that an IP address is well-formed
pub fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>().is_ok()
}

/// Validate IPv4
pub fn is_valid_ipv4(ip: &str) -> bool {
    ip.parse::<std::net::Ipv4Addr>().is_ok()
}

/// Validate IPv6
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
pub fn normalize_ip(ip: &str) -> Option<String> {
    match ip.parse::<std::net::IpAddr>() {
        Ok(addr) => Some(addr.to_string()),
        Err(_) => None,
    }
}

/// Normalize a hostname (lowercase, trim)
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

/// Check if running as root (needed for /etc/hosts modification)
pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
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
}
