use std::fs;
use std::io::{self, Write};
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

    let existing = fs::read_to_string(&target_path).unwrap_or_default();
    let preserved = remove_previous_merges(&existing);

    let mut output = preserved.trim_end().to_string();
    output.push_str("\n\n");
    output.push_str(&merged.join("\n\n"));

    let tmp = target_path.with_extension("tmp");
    {
        let mut f = match fs::File::create(&tmp) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {}", e);
                return false;
            }
        };
        if f.write_all(output.as_bytes()).is_err() {
            return false;
        }
        let _ = f.flush();
        let _ = f.sync_all();
    }
    if fs::rename(&tmp, &target_path).is_err() {
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
        ureq::get(src)
            .call()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
            .into_string()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    } else {
        fs::read_to_string(src)
    }
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
