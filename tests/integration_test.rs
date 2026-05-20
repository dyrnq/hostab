use std::process::Command;

fn hostab(args: &[&str]) -> String {
    let bin = if cfg!(windows) {
        "target/release/hostab.exe"
    } else {
        "target/release/hostab"
    };
    let output = Command::new(bin)
        .args(args)
        .output()
        .expect("failed to run hostab");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn hostab_with_file(hosts_file: &str, args: &[&str]) -> String {
    let bin = if cfg!(windows) {
        "target/release/hostab.exe"
    } else {
        "target/release/hostab"
    };
    let mut all_args = vec!["--hosts-file", hosts_file];
    all_args.extend(args);
    let output = Command::new(bin)
        .args(&all_args)
        .output()
        .expect("failed to run hostab");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_entry_list() {
    let out = hostab_with_file("tests/hosts.test", &["e", "list", "-o", "raw"]);
    assert!(out.contains("127.0.0.1"), "should contain localhost IP");
    assert!(out.contains("app.local"), "should contain app.local");
}

#[test]
fn test_ipv4_filter() {
    let out = hostab_with_file("tests/hosts.test", &["e", "list", "--ipv4", "-o", "raw"]);
    assert!(out.contains("127.0.0.1"), "should have IPv4");
    assert!(!out.contains("::1"), "should not have IPv6");
}

#[test]
fn test_ipv6_filter() {
    let out = hostab_with_file("tests/hosts.test", &["e", "list", "--ipv6", "-o", "raw"]);
    assert!(out.contains("::1"), "should have IPv6");
}

#[test]
fn test_json_output() {
    let out = hostab_with_file("tests/hosts.test", &["-o", "json", "e", "list"]);
    assert!(out.starts_with('['), "should be JSON array");
}

#[test]
fn test_add_entry() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "").unwrap();

    hostab_with_file(path, &["e", "add", "10.99.0.10", "test-add.local", "-q"]);
    let out = hostab_with_file(path, &["e", "list", "-o", "raw"]);
    assert!(out.contains("test-add.local"), "should have added entry");
}

#[test]
fn test_rm_entry() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.99.0.10 test-rm.local\n").unwrap();

    hostab_with_file(path, &["e", "rm", "test-rm.local", "-q"]);
    let out = hostab_with_file(path, &["e", "list", "-o", "raw"]);
    assert!(!out.contains("test-rm.local"), "should have removed entry");
}

#[test]
fn test_rm_by_ip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.99.0.20 ip-rm.local\n").unwrap();

    hostab_with_file(path, &["e", "rm", "--ip", "10.99.0.20", "-q"]);
    let out = hostab_with_file(path, &["e", "list", "-o", "raw"]);
    assert!(!out.contains("ip-rm.local"), "should have removed by IP");
}

#[test]
fn test_filter() {
    let out = hostab_with_file(
        "tests/hosts.test",
        &["e", "list", "-f", "prod", "-o", "raw"],
    );
    assert!(out.contains("prod.local"), "should match substring");
}

#[test]
fn test_disable_enable() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local api.local\n").unwrap();

    hostab_with_file(path, &["e", "disable", "api.local", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(
        raw.contains("# 10.0.0.1 api.local"),
        "should comment out api.local"
    );
    assert!(raw.contains("10.0.0.1 app.local"), "should keep app.local");

    hostab_with_file(path, &["e", "enable", "api.local", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("app.local api.local"), "should merge back");
}

#[test]
fn test_toggle() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    hostab_with_file(path, &["e", "toggle", "app.local", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("# 10.0.0.1 app.local"), "toggle off");

    hostab_with_file(path, &["e", "toggle", "app.local", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("10.0.0.1 app.local"), "toggle on");
}

#[test]
fn test_edit() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local api.local\n").unwrap();

    hostab_with_file(path, &["e", "edit", "api.local", "--ip", "10.0.0.99", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(
        raw.contains("10.0.0.1 app.local"),
        "old IP should keep app.local"
    );
    assert!(
        raw.contains("10.0.0.99 api.local"),
        "new IP should have api.local"
    );
}

#[test]
fn test_verify() {
    let out = hostab_with_file("tests/hosts.test", &["verify"]);
    assert!(
        out.contains("localhost"),
        "should report duplicate localhost"
    );
    assert!(out.contains("LINE"), "should have table header");
}

#[test]
fn test_merge() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    std::fs::write(&a, "10.99.0.1 merge-a.local\n").unwrap();
    let b = dir.path().join("b");
    std::fs::write(&b, "10.99.0.2 merge-b.local\n").unwrap();
    let tgt = dir.path().join("merged");

    hostab(&[
        "merge",
        "-s",
        a.to_str().unwrap(),
        "-s",
        b.to_str().unwrap(),
        "-t",
        tgt.to_str().unwrap(),
        "-q",
    ]);
    let raw = std::fs::read_to_string(&tgt).unwrap();
    assert!(raw.contains("merge-a.local"), "should have file a content");
    assert!(raw.contains("### source:"), "should have annotation");
}

#[test]
fn test_cat() {
    let out = hostab_with_file("tests/hosts.test", &["cat"]);
    assert!(out.contains("localhost"), "should show raw content");
}

#[test]
fn test_completion() {
    let out = hostab(&["completion", "bash"]);
    assert!(out.contains("complete"), "should generate bash completion");
}

#[test]
fn test_version() {
    let out = hostab(&["version"]);
    assert!(out.contains("hostab"), "should show name");
    assert!(out.contains("commit"), "should show commit info");
}
