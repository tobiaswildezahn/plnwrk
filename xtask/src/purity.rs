//! Purity rule of docs/PLAN.md §2: core and engine take no input from the outside world.

use std::path::Path;

/// Source folders that must stay pure.
pub const PURE_CRATES: &[&str] = &["crates/core/src", "crates/engine/src"];

const FORBIDDEN: &[(&str, &str)] = &[
    ("std::fs", "file system"),
    ("std::net", "network"),
    ("std::process", "processes"),
    ("std::env", "environment"),
    ("std::thread", "threads"),
    ("SystemTime", "wall clock"),
    ("Instant::now", "clock"),
    ("rand::", "randomness"),
];

/// Reports every line of `contents` that reaches the outside world. Comment lines are skipped
/// so documentation may name what is forbidden.
pub fn scan(path: &Path, contents: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for (pattern, what) in FORBIDDEN {
            if line.contains(pattern) {
                found.push(format!(
                    "{}:{}: {what} ({pattern}) is not allowed in a pure crate",
                    path.display(),
                    index + 1
                ));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_code_passes() {
        assert!(
            scan(
                Path::new("lib.rs"),
                "pub fn add(a: u8, b: u8) -> u8 { a + b }"
            )
            .is_empty()
        );
    }

    #[test]
    fn file_system_access_is_reported_with_line() {
        let found = scan(Path::new("a.rs"), "fn f() {}\nuse std::fs::read;\n");
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("a.rs:2"));
        assert!(found[0].contains("std::fs"));
    }

    #[test]
    fn clock_randomness_threads_env_net_process_are_reported() {
        for bad in [
            "std::time::SystemTime::now()",
            "Instant::now()",
            "std::thread::spawn(f)",
            "std::env::var(\"X\")",
            "std::net::TcpStream",
            "std::process::Command",
            "rand::random()",
        ] {
            assert_eq!(scan(Path::new("x.rs"), bad).len(), 1, "{bad}");
        }
    }

    #[test]
    fn comments_are_ignored() {
        assert!(
            scan(
                Path::new("x.rs"),
                "    // never call std::fs here\n/// nor std::env"
            )
            .is_empty()
        );
    }

    #[test]
    fn crlf_line_numbers_are_correct() {
        let found = scan(
            Path::new("w.rs"),
            "ok\r\nok\r\nuse std::net::UdpSocket;\r\n",
        );
        assert!(found[0].contains("w.rs:3"));
    }
}
