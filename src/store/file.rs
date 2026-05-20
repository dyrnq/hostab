use std::fs;
use std::io::{self, Write};
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
        Self { path: path.to_path_buf() }
    }

    pub fn with_default_path() -> Self {
        Self::new(Path::new(DEFAULT_HOSTS_PATH))
    }

    /// Load all entries
    pub fn load(&self) -> io::Result<Vec<Entry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path)?;
        Ok(parser::parse(&content))
    }

    /// Save entries atomically
    pub fn save(&self, entries: &[Entry]) -> io::Result<()> {
        validation::validate_secure_path(&self.path)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serializer::serialize(entries);
        let tmp = self.path.with_extension("tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(content.as_bytes())?;
            f.flush()?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
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
    pub fn all_rows(&self) -> io::Result<Vec<Row>> {
        let entries = self.load()?;
        let mut rows = Vec::new();
        for entry in &entries {
            if entry.hostnames.is_empty() {
                continue;
            }
            for host in &entry.hostnames {
                rows.push(Row {
                    ip: entry.ip.clone(),
                    host: host.clone(),
                    comment: entry.comment.clone(),
                });
            }
        }
        Ok(rows)
    }

    /// Add/merge an entry
    pub fn add_entry(&self, ip: &str, hosts: &[String], comment: Option<&str>) -> io::Result<Vec<String>> {
        let mut entries = self.load()?;
        let mut duplicates = Vec::new();

        // Check for duplicate hostnames on other IPs
        for entry in &entries {
            if entry.ip != ip {
                for h in hosts {
                    if entry.hostnames.contains(h) && !duplicates.contains(h) {
                        duplicates.push(h.clone());
                    }
                }
            }
        }

        // Merge into existing entry with same IP
        if let Some(existing) = entries.iter_mut().find(|e| e.ip == ip) {
            for h in hosts {
                if !existing.hostnames.contains(h) {
                    existing.hostnames.push(h.clone());
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
                hostnames: hosts.to_vec(),
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
            let before = entry.hostnames.len();
            entry.hostnames.retain(|h| !hostnames.contains(h));
            removed += before - entry.hostnames.len();
        }
        entries.retain(|e| !e.hostnames.is_empty());

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
            entry.hostnames.retain(|h| {
                if hostnames.contains(h) { split.push(h.clone()); false } else { true }
            });
            if !split.is_empty() {
                for h in &split {
                    next_id += 1;
                    new_entries.push(Entry {
                        id: next_id, ip: entry.ip.clone(), hostnames: vec![h.clone()],
                        comment: entry.comment.clone(), disabled: true, raw: None,
                    });
                    count += 1;
                }
            }
        }
        entries.retain(|e| !e.hostnames.is_empty());
        entries.append(&mut new_entries);
        self.safe_save(&entries)?;
        Ok(count)
    }

    /// Enable hostnames: merge disabled entries back into their IP line.
    pub fn enable_hostname(&self, hostnames: &[String]) -> io::Result<usize> {
        let mut entries = self.load()?;
        let mut count = 0;
        let mut merge: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

        entries.retain(|e| {
            if e.disabled && e.hostnames.iter().any(|h| hostnames.contains(h)) {
                for h in &e.hostnames {
                    if hostnames.contains(h) {
                        merge.entry(e.ip.clone()).or_default().push(h.clone());
                        count += 1;
                    }
                }
                false
            } else {
                true
            }
        });

        for (ip, hosts) in &merge {
            if let Some(existing) = entries.iter_mut().find(|e| e.ip == *ip && !e.disabled) {
                for h in hosts { if !existing.hostnames.contains(h) { existing.hostnames.push(h.clone()); } }
            } else {
                let id = entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
                entries.push(Entry { id, ip: ip.clone(), hostnames: hosts.clone(), comment: None, disabled: false, raw: None });
            }
        }

        self.safe_save(&entries)?;
        Ok(count)
    }

    /// Toggle a hostname: if any entry with it is enabled → disable; else → enable.
    pub fn toggle_hostname(&self, hostname: &str) -> io::Result<String> {
        let entries = self.load()?;
        let any_enabled = entries.iter().any(|e| !e.disabled && e.hostnames.contains(&hostname.to_string()));
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
        if count > 0 { self.safe_save(&entries)?; }
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
        if count > 0 { self.safe_save(&entries)?; }
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
            if entry.hostnames.contains(&hostname.to_string()) {
                entry.hostnames.retain(|h| h != hostname);
                to_add.push((entry.disabled, entry.comment.clone()));
                moved += 1;
            }
        }
        entries.retain(|e| !e.hostnames.is_empty());

        if moved == 0 { return Ok(0); }

        // Add to new IP
        let was_disabled = to_add.iter().any(|(d, _)| *d);
        let comment = to_add.into_iter().find_map(|(_, c)| c);
        if let Some(existing) = entries.iter_mut().find(|e| e.ip == new_ip) {
            if !existing.hostnames.contains(&hostname.to_string()) {
                existing.hostnames.push(hostname.to_string());
            }
            if was_disabled { existing.disabled = true; }
        } else {
            let id = entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
            entries.push(Entry {
                id, ip: new_ip.to_string(), hostnames: vec![hostname.to_string()],
                comment, disabled: was_disabled, raw: None,
            });
        }

        self.safe_save(&entries)?;
        Ok(moved)
    }

    /// Verify the hosts file. Returns structured issues (line, ip, host, issue).
    pub fn verify(&self) -> io::Result<Vec<(usize, String, String, String)>> {
        let content = match fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };

        let mut issues: Vec<(usize, String, String, String)> = Vec::new();
        let mut seen: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();

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
                issues.push((ln, ip.clone(), "-".into(), format!("invalid IP")));
            }
            if parts.len() < 2 {
                issues.push((ln, ip.clone(), "-".into(), format!("missing hostname")));
                continue;
            }
            for host in &parts[1..] {
                let host = host.trim();
                if host.starts_with('#') { break; }
                if !validation::is_valid_hostname(host) {
                    issues.push((ln, ip.clone(), host.to_string(), format!("invalid hostname")));
                }
                seen.entry(host.to_lowercase()).or_default().push(ln);
            }
        }

        for (hostname, lines) in &seen {
            if lines.len() > 1 {
                for &ln in lines {
                    // Find the line content
                    if let Some(raw) = content.lines().nth(ln - 1) {
                        let parts: Vec<&str> = raw.trim().split_whitespace().collect();
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

        store.add_entry("10.0.0.1", &["app.local".into()], None).unwrap();
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

        store.add_entry("10.0.0.1", &["a.local".into()], None).unwrap();
        store.add_entry("10.0.0.2", &["b.local".into()], None).unwrap();
        assert_eq!(store.load().unwrap().len(), 2);

        store.remove_by_ip("10.0.0.1").unwrap();
        assert_eq!(store.load().unwrap().len(), 1);
    }
}
