//! Purity rule of docs/PLAN.md §2: core and engine take no input from the outside world.

use std::path::Path;

/// Source folders that must stay pure.
pub const PURE_CRATES: &[&str] = &["crates/core/src", "crates/engine/src"];

/// Text patterns as a backstop. The precise check is clippy's `disallowed-methods` /
/// `disallowed-types` in `crates/{core,engine}/clippy.toml`, which sees through renamed imports.
const FORBIDDEN: &[(&str, &str)] = &[
    ("std::fs", "file system"),
    ("std::net", "network"),
    ("std::process", "processes"),
    ("std::env", "environment"),
    ("std::thread", "threads"),
    ("std::{", "grouped std import (list the items one per line)"),
    ("fs,", "file system"),
    ("stdin", "standard input"),
    ("SystemTime", "wall clock"),
    ("Instant::now", "clock"),
    ("Timestamp::now", "clock"),
    ("Zoned::now", "clock"),
    ("rand::", "randomness"),
    ("getrandom", "randomness"),
    ("HashMap", "random iteration order (use BTreeMap)"),
    ("HashSet", "random iteration order (use BTreeSet)"),
    ("include_str!", "file contents"),
    ("include_bytes!", "file contents"),
];

/// Reports every line of `contents` that reaches the outside world. Comment lines are skipped
/// so documentation may name what is forbidden.
/// `pattern` occurs in `code` and is not glued to a preceding identifier character.
fn contains_word(code: &str, pattern: &str) -> bool {
    code.match_indices(pattern).any(|(at, _)| {
        code.get(..at)
            .and_then(|before| before.chars().next_back())
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

pub fn scan(path: &Path, contents: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for (pattern, what) in FORBIDDEN {
            if contains_word(code, pattern) {
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
    fn grouped_imports_and_other_clocks_are_reported() {
        for bad in [
            "use std::{env, fs};",
            "    fs,",
            "let t = jiff::Timestamp::now();",
            "let z = Zoned::now();",
            "getrandom::fill(&mut buf)",
            "let m: HashMap<u8, u8> = HashMap::new();",
            "let s = std::io::stdin();",
            "include_str!(\"/etc/hosts\")",
        ] {
            assert!(!scan(Path::new("x.rs"), bad).is_empty(), "{bad}");
        }
    }

    #[test]
    fn patterns_match_whole_words_only() {
        assert!(scan(Path::new("x.rs"), "let (refs, x) = pair;").is_empty());
        assert!(scan(Path::new("x.rs"), "let my_stdin_like = 1;").is_empty());
        assert_eq!(scan(Path::new("x.rs"), "io::stdin()").len(), 1);
        assert!(scan(Path::new("x.rs"), "brand::new()").is_empty());
    }

    #[test]
    fn trailing_comments_are_ignored() {
        assert!(scan(Path::new("x.rs"), "let a = 1; // not std::fs").is_empty());
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
