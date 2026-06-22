use std::fs;
use std::fs::Permissions;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::core::model::{Entry, Row};
use crate::core::parser;
use crate::core::serializer;
use crate::store::lock::FileLock;
use crate::util::validation;

#[cfg(target_os = "linux")]
pub const DEFAULT_HOSTS_PATH: &str = "/etc/hosts";
#[cfg(target_os = "macos")]
pub const DEFAULT_HOSTS_PATH: &str = "/etc/hosts";
#[cfg(target_os = "windows")]
pub const DEFAULT_HOSTS_PATH: &str = "C:/Windows/System32/Drivers/etc/hosts";

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    /// Load all entries
    pub fn load(&self) -> io::Result<Vec<Entry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        validation::validate_secure_path(&self.path)?;
        let content = fs::read_to_string(&self.path)?;
        Ok(parser::parse(&content))
    }

    /// Save entries atomically
    pub fn save(&self, entries: &[Entry]) -> io::Result<()> {
        validation::validate_secure_path(&self.path)?;
        // Snapshot original permissions before creating/removing any files
        let orig_perms = fs::metadata(&self.path)
            .ok()
            .map(|m| m.permissions().mode());
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serializer::serialize(entries);
        // Use temporary file in the same directory with a random suffix to avoid TOCTOU
        let dir = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir)
            .map_err(|e| io::Error::other(format!("failed to create temp file: {}", e)))?;
        #[cfg(unix)]
        {
            // Preserve original file permissions so regular users can still read
            // /etc/hosts after root saves it. Default to 0644 if file doesn't exist.
            let mode = orig_perms.unwrap_or(0o100644);
            let perms = Permissions::from_mode(mode & 0o777);
            tmp.as_file()
                .set_permissions(perms)
                .map_err(|e| io::Error::other(format!("failed to set permissions: {}", e)))?;
        }
        tmp.write_all(content.as_bytes())?;
        tmp.flush()?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.path)
            .map_err(|e| io::Error::other(format!("failed to persist temp file: {}", e)))?;
        Ok(())
    }

    /// Save with locking
    pub fn safe_save(&self, entries: &[Entry]) -> io::Result<()> {
        let mut lock = FileLock::new(&self.path);
        lock.lock()?;
        let result = self.save(entries);
        lock.unlock()?;
        result
    }

    /// Get all entries as display rows
    #[allow(dead_code)]
    pub fn all_rows(&self) -> io::Result<Vec<Row>> {
        let entries = self.load()?;
        let mut rows = Vec::new();
        for entry in &entries {
            if entry.canonical.is_empty() && entry.aliases.is_empty() {
                continue;
            }
            // canonical first, then aliases
            rows.push(Row {
                ip: entry.ip.clone(),
                host: entry.canonical.clone(),
                comment: entry.comment.clone(),
                canonical: Some(entry.canonical.clone()),
                aliases: entry.aliases.clone(),
            });
        }
        Ok(rows)
    }

    /// Add/merge an entry
    pub fn add_entry(
        &self,
        ip: &str,
        hosts: &[String],
        comment: Option<&str>,
    ) -> io::Result<Vec<String>> {
        let mut entries = self.load()?;
        let mut duplicates = Vec::new();

        // Check for duplicate hostnames on other IPs
        for entry in &entries {
            if entry.ip != ip {
                for h in hosts {
                    if (entry.canonical == *h || entry.aliases.contains(h))
                        && !duplicates.contains(h)
                    {
                        duplicates.push(h.clone());
                    }
                }
            }
        }

        // If the canonical hostname differs from existing entry, create a new line
        let canonical_host = hosts.first().map(|s| s.as_str()).unwrap_or("");
        let existing = entries
            .iter()
            .position(|e| e.ip == ip && e.canonical == canonical_host);

        if let Some(idx) = existing {
            let existing = &mut entries[idx];
            for h in hosts {
                if existing.canonical != *h && !existing.aliases.contains(h) {
                    existing.aliases.push(h.clone());
                }
            }
            if let Some(c) = comment {
                existing.comment = Some(c.to_string());
            }
        } else {
            let next_id = entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
            entries.push(Entry {
                id: next_id,
                ip: ip.to_string(),
                canonical: hosts.first().cloned().unwrap_or_default(),
                aliases: hosts.get(1..).map(|s| s.to_vec()).unwrap_or_default(),
                comment: comment.map(|c| c.to_string()),
                disabled: false,
                raw: None,
            });
        }

        self.safe_save(&entries)?;
        Ok(duplicates)
    }

    /// Remove hostnames from all entries
    pub fn remove_hostnames(&self, hostnames: &[String]) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut removed = 0;

        for entry in &mut entries {
            let host_count_before =
                (if entry.canonical.is_empty() { 0 } else { 1 }) + entry.aliases.len();

            // If canonical matches, promote first alias
            if !entry.canonical.is_empty() && hostnames.contains(&entry.canonical) {
                if !entry.aliases.is_empty() {
                    entry.canonical = entry.aliases.remove(0);
                } else {
                    entry.canonical.clear();
                }
            }
            // Remove from aliases
            entry.aliases.retain(|h| !hostnames.contains(h));

            let host_count_after =
                (if entry.canonical.is_empty() { 0 } else { 1 }) + entry.aliases.len();
            removed += host_count_before - host_count_after;
        }
        entries.retain(|e| !e.canonical.is_empty() || !e.aliases.is_empty());

        self.safe_save(&entries)?;
        Ok(removed)
    }

    /// Remove all entries for an IP
    pub fn remove_by_ip(&self, ip: &str) -> io::Result<usize> {
        let mut entries = self.load()?;
        let before = entries.len();
        entries.retain(|e| e.ip != ip);
        let removed = before - entries.len();
        self.safe_save(&entries)?;
        Ok(removed)
    }

    /// Disable hostnames: split each out into its own disabled line.
    pub fn disable_hostname(&self, hostnames: &[String]) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut count = 0;
        let mut new_entries: Vec<Entry> = Vec::new();
        let mut next_id = entries.iter().map(|e| e.id).max().unwrap_or(0);

        for entry in &mut entries {
            let mut split: Vec<String> = Vec::new();

            // Check canonical
            if hostnames.contains(&entry.canonical) {
                split.push(entry.canonical.clone());
                if !entry.aliases.is_empty() {
                    entry.canonical = entry.aliases.remove(0);
                } else {
                    entry.canonical.clear();
                }
            }

            // Check aliases
            entry.aliases.retain(|h| {
                if hostnames.contains(h) {
                    split.push(h.clone());
                    false
                } else {
                    true
                }
            });

            if !split.is_empty() {
                for h in &split {
                    next_id += 1;
                    new_entries.push(Entry {
                        id: next_id,
                        ip: entry.ip.clone(),
                        canonical: h.clone(),
                        aliases: vec![],
                        comment: entry.comment.clone(),
                        disabled: true,
                        raw: None,
                    });
                    count += 1;
                }
            }
        }
        entries.retain(|e| !e.canonical.is_empty() || !e.aliases.is_empty());
        entries.append(&mut new_entries);
        self.safe_save(&entries)?;
        Ok(count)
    }

    /// Enable hostnames: merge disabled entries back into their IP line.
    pub fn enable_hostname(&self, hostnames: &[String]) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut count = 0;
        let mut merge: Vec<(String, String)> = Vec::new(); // (ip, hostname)
        let mut kept_disabled: Vec<Entry> = Vec::new();

        entries.retain(|e| {
            if e.disabled {
                let mut matched = false;
                let mut to_merge: Vec<String> = Vec::new();
                let mut remaining: Vec<String> = Vec::new();

                if hostnames.contains(&e.canonical) {
                    matched = true;
                    to_merge.push(e.canonical.clone());
                } else if !e.canonical.is_empty() {
                    remaining.push(e.canonical.clone());
                }

                for h in &e.aliases {
                    if hostnames.contains(h) {
                        matched = true;
                        to_merge.push(h.clone());
                    } else {
                        remaining.push(h.clone());
                    }
                }

                if matched {
                    for h in to_merge {
                        merge.push((e.ip.clone(), h));
                        count += 1;
                    }
                    if !remaining.is_empty() {
                        kept_disabled.push(Entry {
                            id: 0,
                            ip: e.ip.clone(),
                            canonical: remaining.remove(0),
                            aliases: remaining,
                            comment: e.comment.clone(),
                            disabled: true,
                            raw: None,
                        });
                    }
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        {
            let mut id = entries.iter().map(|e| e.id).max().unwrap_or(0);
            for entry in &mut kept_disabled {
                id += 1;
                entry.id = id;
            }
        }
        entries.append(&mut kept_disabled);

        for (ip, hostname) in &merge {
            if let Some(existing) = entries.iter_mut().find(|e| e.ip == *ip && !e.disabled) {
                if existing.canonical != *hostname && !existing.aliases.contains(hostname) {
                    existing.aliases.push(hostname.clone());
                }
            } else {
                let id = entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
                entries.push(Entry {
                    id,
                    ip: ip.clone(),
                    canonical: hostname.clone(),
                    aliases: vec![],
                    comment: None,
                    disabled: false,
                    raw: None,
                });
            }
        }

        self.safe_save(&entries)?;
        Ok(count)
    }

    /// Toggle a hostname: if any entry with it is enabled → disable; else → enable.
    pub fn toggle_hostname(&self, hostname: &str) -> io::Result<String> {
        let entries = self.load()?;
        let any_enabled = entries.iter().any(|e| {
            !e.disabled && (e.canonical == *hostname || e.aliases.contains(&hostname.to_string()))
        });
        if any_enabled {
            self.disable_hostname(&[hostname.to_string()])?;
            Ok(format!("disabled {}", hostname))
        } else {
            self.enable_hostname(&[hostname.to_string()])?;
            Ok(format!("enabled {}", hostname))
        }
    }

    /// Disable all entries for an IP
    pub fn disable_by_ip(&self, ip: &str) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut count = 0;
        for entry in &mut entries {
            if entry.ip == ip && !entry.disabled {
                entry.disabled = true;
                count += 1;
            }
        }
        if count > 0 {
            self.safe_save(&entries)?;
        }
        Ok(count)
    }

    /// Enable all disabled entries for an IP
    pub fn enable_by_ip(&self, ip: &str) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut count = 0;
        for entry in &mut entries {
            if entry.ip == ip && entry.disabled {
                entry.disabled = false;
                count += 1;
            }
        }
        if count > 0 {
            self.safe_save(&entries)?;
        }
        Ok(count)
    }

    /// Toggle all entries for an IP
    pub fn toggle_by_ip(&self, ip: &str) -> io::Result<String> {
        let entries = self.load()?;
        let any_enabled = entries.iter().any(|e| e.ip == ip && !e.disabled);
        if any_enabled {
            let n = self.disable_by_ip(ip)?;
            Ok(format!("disabled {} entry(s) for IP {}", n, ip))
        } else {
            let n = self.enable_by_ip(ip)?;
            Ok(format!("enabled {} entry(s) for IP {}", n, ip))
        }
    }

    /// Move a hostname from its current IP(s) to a new IP
    pub fn move_hostname(&self, hostname: &str, new_ip: &str) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut moved = 0;
        let mut to_add: Vec<(bool, Option<String>)> = Vec::new(); // (was_disabled, comment)

        // Remove from old entries
        for entry in &mut entries {
            let in_canonical = entry.canonical == hostname;
            let in_aliases = entry.aliases.contains(&hostname.to_string());
            if in_canonical || in_aliases {
                if in_canonical {
                    if !entry.aliases.is_empty() {
                        entry.canonical = entry.aliases.remove(0);
                    } else {
                        entry.canonical.clear();
                    }
                }
                if in_aliases {
                    entry.aliases.retain(|h| h != hostname);
                }
                to_add.push((entry.disabled, entry.comment.clone()));
                moved += 1;
            }
        }
        entries.retain(|e| !e.canonical.is_empty() || !e.aliases.is_empty());

        if moved == 0 {
            return Ok(0);
        }

        // Add to new IP
        let was_disabled = to_add.iter().all(|(d, _)| *d);
        let comment = to_add.into_iter().find_map(|(_, c)| c);
        if let Some(existing) = entries.iter_mut().find(|e| e.ip == new_ip) {
            if existing.canonical != hostname && !existing.aliases.contains(&hostname.to_string()) {
                existing.aliases.push(hostname.to_string());
            }
            if was_disabled {
                existing.disabled = true;
            }
        } else {
            let id = entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
            entries.push(Entry {
                id,
                ip: new_ip.to_string(),
                canonical: hostname.to_string(),
                aliases: vec![],
                comment,
                disabled: was_disabled,
                raw: None,
            });
        }

        self.safe_save(&entries)?;
        Ok(moved)
    }

    /// Verify the hosts file. Returns structured issues (line, ip, host, issue).
    pub fn verify(&self) -> io::Result<Vec<(usize, String, String, String)>> {
        validation::validate_secure_path(&self.path)?;
        let content = fs::read_to_string(&self.path)?;

        let mut issues: Vec<(usize, String, String, String)> = Vec::new();
        let mut seen: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();

        for (line_no, line) in content.lines().enumerate() {
            let ln = line_no + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let ip = parts[0].to_string();
            if !validation::is_valid_ip(&ip) {
                issues.push((ln, ip.clone(), "-".into(), "invalid IP".to_string()));
            }
            if parts.len() < 2 {
                issues.push((ln, ip.clone(), "-".into(), "missing hostname".to_string()));
                continue;
            }
            for host in &parts[1..] {
                let host = host.trim();
                if host.starts_with('#') {
                    break;
                }
                if !validation::is_valid_hostname(host) {
                    issues.push((
                        ln,
                        ip.clone(),
                        host.to_string(),
                        "invalid hostname".to_string(),
                    ));
                }
                seen.entry(host.to_lowercase()).or_default().push(ln);
            }
        }

        for (hostname, lines) in &seen {
            if lines.len() > 1 {
                for &ln in lines {
                    // Find the line content
                    if let Some(raw) = content.lines().nth(ln - 1) {
                        let parts: Vec<&str> = raw.split_whitespace().collect();
                        let ip = parts.first().map(|s| s.to_string()).unwrap_or_default();
                        issues.push((ln, ip, hostname.clone(), "duplicate".into()));
                    }
                }
            }
        }

        // Sort by line number
        issues.sort_by_key(|(ln, _, _, _)| *ln);
        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_save_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "127.0.0.1 localhost\n").unwrap();
        let store = Store::new(&path);
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_add_and_rm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);

        store
            .add_entry("10.0.0.1", &["app.local".into()], None)
            .unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);

        let removed = store.remove_hostnames(&["app.local".into()]).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.load().unwrap().len(), 0);
    }

    #[test]
    fn test_rm_by_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);

        store
            .add_entry("10.0.0.1", &["a.local".into()], None)
            .unwrap();
        store
            .add_entry("10.0.0.2", &["b.local".into()], None)
            .unwrap();
        assert_eq!(store.load().unwrap().len(), 2);

        store.remove_by_ip("10.0.0.1").unwrap();
        assert_eq!(store.load().unwrap().len(), 1);
    }

    #[test]
    fn test_add_duplicate_hostname_on_other_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);

        store
            .add_entry("10.0.0.1", &["svc.local".into()], None)
            .unwrap();
        let dups = store
            .add_entry("10.0.0.2", &["svc.local".into()], None)
            .unwrap();
        assert_eq!(dups, vec!["svc.local"]);
    }

    #[test]
    fn test_add_duplicate_hostname_same_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);

        store
            .add_entry("10.0.0.1", &["svc.local".into()], None)
            .unwrap();
        let dups = store
            .add_entry("10.0.0.1", &["svc.local".into()], None)
            .unwrap();
        assert!(dups.is_empty());
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_add_invalid_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);
        // add_entry doesn't validate IP, but we test it stores as-is
        store
            .add_entry("not-an-ip", &["test.local".into()], None)
            .unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries[0].ip, "not-an-ip");
    }

    #[test]
    fn test_add_empty_hostnames() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);
        let empty: Vec<String> = vec![];
        store.add_entry("10.0.0.1", &empty, None).unwrap();
        // Empty hostnames produce "10.0.0.1 \n" which cannot round-trip
        // through the parser (regex requires at least one hostname after IP).
        // The entry is created but lost after save/reload.
        let entries = store.load().unwrap();
        assert!(
            entries.is_empty(),
            "empty-hostname entry cannot survive save/reload"
        );
    }

    #[test]
    fn test_remove_non_existent_hostname() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);
        let removed = store.remove_hostnames(&["none.local".into()]).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(store.load().unwrap().len(), 1);
    }

    #[test]
    fn test_remove_by_non_existent_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);
        let removed = store.remove_by_ip("10.0.0.99").unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_disable_then_remove() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local api.local\n").unwrap();
        let store = Store::new(&path);

        store.disable_hostname(&["api.local".into()]).unwrap();
        let entries = store.load().unwrap();
        // Should have 2 entries: enabled app.local, disabled api.local
        let disabled: Vec<_> = entries.iter().filter(|e| e.disabled).collect();
        assert_eq!(disabled.len(), 1);

        store.remove_hostnames(&["api.local".into()]).unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].disabled);
        assert_eq!(entries[0].canonical, "app.local");
        assert!(entries[0].aliases.is_empty());
    }

    #[test]
    fn test_enable_not_disabled() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);
        let count = store.enable_hostname(&["app.local".into()]).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_disable_not_present() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);
        let count = store.disable_hostname(&["none.local".into()]).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_toggle_non_existent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);
        let msg = store.toggle_hostname("none.local").unwrap();
        assert!(msg.contains("enabled"), "should enable when not found");
    }

    #[test]
    fn test_move_non_existent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);
        let moved = store.move_hostname("none.local", "10.0.0.99").unwrap();
        assert_eq!(moved, 0);
    }

    #[test]
    fn test_verify_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);
        let issues = store.verify().unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_verify_missing_hostname() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "127.0.0.1\n").unwrap();
        let store = Store::new(&path);
        let issues = store.verify().unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].3.contains("missing hostname"));
    }

    #[test]
    fn test_load_non_existent_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent");
        let store = Store::new(&path);
        let entries = store.load().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_disable_by_then_enable_by_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local api.local\n").unwrap();
        let store = Store::new(&path);

        let n = store.disable_by_ip("10.0.0.1").unwrap();
        assert_eq!(n, 1);

        let entries = store.load().unwrap();
        assert!(entries[0].disabled);

        let n = store.enable_by_ip("10.0.0.1").unwrap();
        assert_eq!(n, 1);

        let entries = store.load().unwrap();
        assert!(!entries[0].disabled);
    }

    #[test]
    fn test_toggle_by_ip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local\n").unwrap();
        let store = Store::new(&path);

        let msg = store.toggle_by_ip("10.0.0.1").unwrap();
        assert!(msg.contains("disabled"));

        let entries = store.load().unwrap();
        assert!(entries[0].disabled);

        let msg = store.toggle_by_ip("10.0.0.1").unwrap();
        assert!(msg.contains("enabled"));
    }

    #[test]
    fn test_serialize_remove_reload_cycle() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "").unwrap();
        let store = Store::new(&path);

        store
            .add_entry("10.0.0.1", &["a.local".into(), "b.local".into()], None)
            .unwrap();
        store.disable_hostname(&["b.local".into()]).unwrap();
        store.enable_hostname(&["b.local".into()]).unwrap();
        store.remove_hostnames(&["a.local".into()]).unwrap();

        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].canonical, "b.local");
        assert!(entries[0].aliases.is_empty());
        assert!(!entries[0].disabled);
    }
    #[test]
    fn test_all_rows_canonical() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 app.local api.local\n").unwrap();
        let store = Store::new(&path);
        let rows = store.all_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical, Some("app.local".to_string()));
        assert_eq!(rows[0].aliases, vec!["api.local"]);
    }

    #[test]
    fn test_all_rows_single_hostname() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 solo.local\n").unwrap();
        let store = Store::new(&path);
        let rows = store.all_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical, Some("solo.local".to_string()));
        assert!(rows[0].aliases.is_empty());
    }

    #[test]
    fn test_remove_canonical_promotes_alias() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 primary.local secondary.local\n").unwrap();
        let store = Store::new(&path);
        store.remove_hostnames(&["primary.local".into()]).unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].canonical, "secondary.local");
        assert!(entries[0].aliases.is_empty());
    }

    #[test]
    fn test_remove_alias_only() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 primary.local secondary.local\n").unwrap();
        let store = Store::new(&path);
        store.remove_hostnames(&["secondary.local".into()]).unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].canonical, "primary.local");
        assert!(entries[0].aliases.is_empty());
    }

    #[test]
    fn test_disable_canonical_promotes_alias() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 primary.local secondary.local\n").unwrap();
        let store = Store::new(&path);
        store.disable_hostname(&["primary.local".into()]).unwrap();
        let entries = store.load().unwrap();
        let enabled: Vec<_> = entries.iter().filter(|e| !e.disabled).collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].canonical, "secondary.local");
        let disabled: Vec<_> = entries.iter().filter(|e| e.disabled).collect();
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].canonical, "primary.local");
    }

    #[test]
    fn test_edit_moves_canonical() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 myhost.local\n").unwrap();
        let store = Store::new(&path);
        store.move_hostname("myhost.local", "10.0.0.99").unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].ip, "10.0.0.99");
        assert_eq!(entries[0].canonical, "myhost.local");
    }

    #[test]
    fn test_move_hostname_leaves_other_aliases() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "10.0.0.1 primary.local secondary.local\n").unwrap();
        let store = Store::new(&path);
        store.move_hostname("secondary.local", "10.0.0.99").unwrap();
        let entries = store.load().unwrap();
        let orig: Vec<_> = entries.iter().filter(|e| e.ip == "10.0.0.1").collect();
        assert_eq!(orig.len(), 1);
        assert_eq!(orig[0].canonical, "primary.local");
        let new: Vec<_> = entries.iter().filter(|e| e.ip == "10.0.0.99").collect();
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].canonical, "secondary.local");
    }

    #[test]
    fn test_add_same_ip_different_canonical_creates_new_entry() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hosts");
        fs::write(&path, "55.55.55.55 xxx.com zzz.com\n").unwrap();
        let store = Store::new(&path);
        store
            .add_entry("55.55.55.55", &["yyy.com".into(), "aaa.com".into()], None)
            .unwrap();
        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].canonical, "xxx.com");
        assert_eq!(entries[1].canonical, "yyy.com");
    }
}
