use std::process::Command;

fn hostab_bin() -> &'static str {
    if let Ok(bin) = std::env::var("HOSTAB_BIN") {
        Box::leak(bin.into_boxed_str())
    } else if cfg!(windows) {
        "target/release/hostab.exe"
    } else {
        "target/debug/hostab"
    }
}

fn hostab(args: &[&str]) -> String {
    let output = Command::new(hostab_bin())
        .args(args)
        .output()
        .expect("failed to run hostab");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn hostab_with_file(hosts_file: &str, args: &[&str]) -> String {
    let mut all_args = vec!["--hosts-file", hosts_file];
    all_args.extend(args);
    let output = Command::new(hostab_bin())
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

#[test]
fn test_empty_file_list() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "").unwrap();

    let out = hostab_with_file(path, &["e", "list", "-o", "raw"]);
    // Should handle empty file gracefully (just header)
    assert!(out.contains("IP"), "should output header");
}

#[test]
fn test_filter_no_match() {
    let out = hostab_with_file(
        "tests/hosts.test",
        &["e", "list", "-f", "zxcvbnm", "-o", "raw"],
    );
    assert!(!out.contains("app.local"), "no matches expected");
}

#[test]
fn test_filter_regex_chars() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    // Use separate IPs so compact mode doesn't merge rows
    std::fs::write(path, "10.0.0.1 plain.local\n10.0.0.2 test.local\n").unwrap();

    // Filter `*` (glob) should match all hostnames
    let out = hostab_with_file(path, &["e", "list", "-f", "*", "-o", "raw"]);
    assert!(
        out.contains("plain.local"),
        "wildcard filter should match plain.local"
    );
    assert!(
        out.contains("test.local"),
        "wildcard filter should match test.local"
    );

    // Substring filter (no wildcards) should only match the target
    let out2 = hostab_with_file(path, &["e", "list", "-f", "plain", "-o", "raw"]);
    assert!(
        out2.contains("plain.local"),
        "substring should match plain.local"
    );
    assert!(
        !out2.contains("test.local"),
        "substring should not match test.local"
    );
}

#[test]
fn test_add_with_invalid_ip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "").unwrap();

    // add doesn't validate IP, so add with "999.999.999.999" should still store it
    hostab_with_file(path, &["e", "add", "999.999.999.999", "bad-ip.local", "-q"]);
    let out = hostab_with_file(path, &["e", "list", "-o", "raw"]);
    assert!(out.contains("999.999.999.999"), "stores invalid IP as-is");
}

#[test]
fn test_rm_empties_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 lone.local\n").unwrap();

    hostab_with_file(path, &["e", "rm", "lone.local", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(
        raw.trim().is_empty(),
        "file should be empty after removing last entry"
    );
}

#[test]
fn test_add_merge_existing_ip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    hostab_with_file(
        path,
        &["e", "add", "10.0.0.1", "app.local", "api.local", "-q"],
    );
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("app.local api.local") || raw.contains("api.local app.local"));
}

#[test]
fn test_add_no_duplicate_same_ip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    hostab_with_file(path, &["e", "add", "10.0.0.1", "app.local", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    // Should still only have one app.local
    let count = raw.matches("app.local").count();
    assert_eq!(count, 1, "should not duplicate hostname on same IP");
}

#[test]
fn test_disable_nonexistent() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    // disable a hostname that doesn't exist - should be harmless
    hostab_with_file(path, &["e", "disable", "nonexistent", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(
        raw.contains("10.0.0.1 app.local"),
        "existing entry preserved"
    );
}

#[test]
fn test_enable_nonexistent() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    hostab_with_file(path, &["e", "enable", "nonexistent", "-q"]);
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("app.local"), "existing entry preserved");
}

#[test]
fn test_toggle_roundtrip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    // Toggle 4 times: on -> off -> on -> off -> on
    hostab_with_file(path, &["e", "toggle", "app.local", "-q"]);
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("# 10.0.0.1 app.local"));
    hostab_with_file(path, &["e", "toggle", "app.local", "-q"]);
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("10.0.0.1 app.local\n"));
    hostab_with_file(path, &["e", "toggle", "app.local", "-q"]);
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("# 10.0.0.1 app.local"));
    hostab_with_file(path, &["e", "toggle", "app.local", "-q"]);
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("10.0.0.1 app.local\n"));
}

#[test]
fn test_edit_nonexistent() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1 app.local\n").unwrap();

    hostab_with_file(
        path,
        &["e", "edit", "nonexistent", "--ip", "10.0.0.99", "-q"],
    );
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("app.local"), "original entry preserved");
}

#[test]
fn test_verify_clean() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "127.0.0.1 localhost\n").unwrap();

    let out = hostab_with_file(path, &["verify"]);
    assert!(
        out.contains("No issues found"),
        "clean file reports no issues"
    );
}

#[test]
fn test_verify_invalid_ip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "not-an-ip host.local\n").unwrap();

    let out = hostab_with_file(path, &["verify"]);
    assert!(out.contains("invalid IP"), "reports invalid IP");
}

#[test]
fn test_verify_missing_hostname() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "10.0.0.1\n").unwrap();

    let out = hostab_with_file(path, &["verify"]);
    assert!(out.contains("missing hostname"), "reports missing hostname");
}

#[test]
fn test_merge_empty_source() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    std::fs::write(&a, "").unwrap();
    let tgt = dir.path().join("merged");

    hostab(&[
        "merge",
        "-s",
        a.to_str().unwrap(),
        "-t",
        tgt.to_str().unwrap(),
        "-q",
    ]);
    let raw = std::fs::read_to_string(&tgt).unwrap();
    assert!(
        raw.contains("### source:"),
        "should have annotation even for empty source"
    );
}

#[test]
fn test_cat_empty() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "").unwrap();

    let out = hostab_with_file(path, &["cat"]);
    assert_eq!(out.trim(), "");
}

#[test]
fn test_completion_zsh() {
    let out = hostab(&["completion", "zsh"]);
    assert!(!out.is_empty(), "should generate zsh completion");
}

#[test]
fn test_completion_fish() {
    let out = hostab(&["completion", "fish"]);
    assert!(!out.is_empty(), "should generate fish completion");
}

#[test]
fn test_completion_invalid() {
    hostab(&["completion", "invalid_shell"]);
    // Should handle gracefully - not crash
}

fn start_server(hosts_content: &str, port: u16) -> (tempfile::TempDir, std::process::Child) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hosts");
    std::fs::write(&path, hosts_content).unwrap();

    let child = std::process::Command::new(hostab_bin())
        .args([
            "--hosts-file",
            path.to_str().unwrap(),
            "serve",
            "-p",
            &port.to_string(),
            "-q",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start server");

    std::thread::sleep(std::time::Duration::from_millis(1200));
    (dir, child)
}

fn get_json(url: &str) -> serde_json::Value {
    let resp = ureq::get(url).call().unwrap();
    let body = resp.into_body().read_to_string().unwrap();
    serde_json::from_str(&body).unwrap()
}

fn post_json(url: &str, body: &str) -> (u16, serde_json::Value) {
    let val: serde_json::Value = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(body).unwrap()
    };
    let resp = ureq::post(url).send_json(val).unwrap();
    let status = resp.status().as_u16();
    let s = resp.into_body().read_to_string().unwrap();
    let json: serde_json::Value = serde_json::from_str(&s).unwrap();
    (status, json)
}

fn put_json(url: &str, body: &str) -> (u16, serde_json::Value) {
    let val: serde_json::Value = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(body).unwrap()
    };
    let resp = ureq::put(url).send_json(val).unwrap();
    let status = resp.status().as_u16();
    let s = resp.into_body().read_to_string().unwrap();
    let json: serde_json::Value = serde_json::from_str(&s).unwrap();
    (status, json)
}

fn delete_req(url: &str) -> (u16, serde_json::Value) {
    let resp = ureq::delete(url).call().unwrap();
    let status = resp.status().as_u16();
    let s = resp.into_body().read_to_string().unwrap();
    let json: serde_json::Value = serde_json::from_str(&s).unwrap();
    (status, json)
}

#[test]
fn test_serve_list() {
    let (_dir, mut child) =
        start_server("10.0.0.1 app.local api.local\n127.0.0.1 localhost\n", 17201);
    let json = get_json("http://localhost:17201/api/entries");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["ip"], "10.0.0.1");
    assert_eq!(arr[0]["host"], "app.local api.local");
    assert_eq!(arr[0]["canonical"], "app.local");
    assert_eq!(arr[0]["aliases"], serde_json::json!(["api.local"]));
    assert_eq!(arr[1]["ip"], "127.0.0.1");
    assert_eq!(arr[1]["host"], "localhost");
    assert_eq!(arr[1]["canonical"], "localhost");
    assert_eq!(arr[1]["aliases"], serde_json::json!([]));
    child.kill().unwrap();
}

#[test]
fn test_serve_list_filter_ip() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n10.0.0.2 db.local\n", 17202);
    let json = get_json("http://localhost:17202/api/entries?ip=10.0.0.2");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["host"], "db.local");
    child.kill().unwrap();
}

#[test]
fn test_serve_list_filter_hostname() {
    let (_dir, mut child) =
        start_server("10.0.0.1 app.local api.local\n10.0.0.2 db.local\n", 17203);
    let json = get_json("http://localhost:17203/api/entries?hostname=api.local");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["ip"], "10.0.0.1");
    child.kill().unwrap();
}

#[test]
fn test_serve_add() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n", 17204);
    let (status, _) = post_json(
        "http://localhost:17204/api/entries",
        r#"{"ip":"10.0.0.2","hosts":["db.local"],"comment":"database"}"#,
    );
    assert_eq!(status, 201);

    let json = get_json("http://localhost:17204/api/entries");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[1]["ip"], "10.0.0.2");
    assert_eq!(arr[1]["canonical"], "db.local");
    child.kill().unwrap();
}

#[test]
fn test_serve_add_same_ip_new_canonical() {
    let (_dir, mut child) = start_server("55.55.55.55 xxx.com\n", 17205);
    let (status, _) = post_json(
        "http://localhost:17205/api/entries",
        r#"{"ip":"55.55.55.55","hosts":["yyy.com","aaa.com"]}"#,
    );
    assert_eq!(status, 201);

    let json = get_json("http://localhost:17205/api/entries");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["canonical"], "xxx.com");
    assert_eq!(arr[1]["canonical"], "yyy.com");
    child.kill().unwrap();
}

#[test]
fn test_serve_delete_by_hostname() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local api.local\n", 17206);
    let (status, _) = delete_req("http://localhost:17206/api/entries/api.local");
    assert_eq!(status, 200);

    let json = get_json("http://localhost:17206/api/entries");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["host"], "app.local");
    child.kill().unwrap();
}

#[test]
fn test_serve_delete_by_ip() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n10.0.0.2 db.local\n", 17207);
    let (status, _) = delete_req("http://localhost:17207/api/entries?ip=10.0.0.2");
    assert_eq!(status, 200);

    let json = get_json("http://localhost:17207/api/entries");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["ip"], "10.0.0.1");
    child.kill().unwrap();
}

#[test]
fn test_serve_disable_enable() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n", 17208);
    let (status, _) = put_json("http://localhost:17208/api/entries/app.local/disable", "");
    assert_eq!(status, 200);

    let json = get_json("http://localhost:17208/api/entries");
    assert!(
        json.as_array().unwrap().is_empty(),
        "disabled entry should not appear"
    );

    let (status, _) = put_json("http://localhost:17208/api/entries/app.local/enable", "");
    assert_eq!(status, 200);

    let json2 = get_json("http://localhost:17208/api/entries");
    assert_eq!(json2.as_array().unwrap().len(), 1);
    child.kill().unwrap();
}

#[test]
fn test_serve_edit_move() {
    let (_dir, mut child) = start_server("10.0.0.1 myhost.local\n", 17209);
    let (status, _) = put_json(
        "http://localhost:17209/api/entries/myhost.local",
        r#"{"ip":"10.0.0.99"}"#,
    );
    assert_eq!(status, 200);

    let json = get_json("http://localhost:17209/api/entries");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["ip"], "10.0.0.99");
    assert_eq!(arr[0]["canonical"], "myhost.local");
    child.kill().unwrap();
}

#[test]
fn test_serve_not_found() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n", 17210);
    let (status, json) = delete_req("http://localhost:17210/api/entries/nonexistent");
    assert_eq!(status, 200);
    assert_eq!(json["count"], 0);
    child.kill().unwrap();
}

#[test]
fn test_serve_openapi_spec() {
    let (_dir, mut child) = start_server("127.0.0.1 localhost\n", 17211);
    let json = get_json("http://localhost:17211/api/openapi.json");
    assert_eq!(json["openapi"], "3.1.0");
    assert!(json["paths"].is_object());
    child.kill().unwrap();
}

#[test]
fn test_serve_not_found_404() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n", 17212);
    let result = ureq::put("http://localhost:17212/api/entries/nonexistent")
        .send_json(serde_json::json!({"ip": "10.0.0.99"}));
    match result {
        Err(ureq::Error::StatusCode(404)) => {}
        Err(e) => panic!("unexpected error: {}", e),
        Ok(r) => panic!("expected 404, got {}", r.status()),
    }
    child.kill().unwrap();
}

#[test]
fn test_serve_toggle() {
    let (_dir, mut child) = start_server("10.0.0.1 app.local\n", 17213);
    let resp = ureq::put("http://localhost:17213/api/entries/app.local/toggle")
        .send_json(serde_json::Value::Null)
        .unwrap();
    assert_eq!(resp.status(), 200);
    let s = resp.into_body().read_to_string().unwrap();
    let json: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(json["message"].as_str().unwrap().contains("disabled"));

    let json2 = get_json("http://localhost:17213/api/entries");
    assert!(
        json2.as_array().unwrap().is_empty(),
        "toggled off entry should not appear"
    );
    child.kill().unwrap();
}
