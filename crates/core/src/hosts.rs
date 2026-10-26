//! Blok domain DevLock di file hosts Windows.

use std::path::PathBuf;

const BEGIN: &str = "# devlock-start (dikelola DevLock, jangan diedit manual)";
const END: &str = "# devlock-end";

pub fn hosts_file() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(root).join("System32\\drivers\\etc\\hosts")
}

/// Ganti (atau tambahkan) blok DevLock di isi file hosts. Domain kosong = blok dihapus.
pub fn apply(content: &str, domains: &[String]) -> String {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in content.lines() {
        match line.trim() {
            l if l == BEGIN || l.starts_with("# devlock-start") => inside = true,
            END => inside = false,
            _ if !inside => out.push(line),
            _ => {}
        }
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }

    let mut text = out.join(newline);
    if !domains.is_empty() {
        if !text.is_empty() {
            text.push_str(newline);
            text.push_str(newline);
        }
        text.push_str(BEGIN);
        for domain in domains {
            text.push_str(&format!("{newline}127.0.0.1 {domain}{newline}::1 {domain}"));
        }
        text.push_str(newline);
        text.push_str(END);
    }
    text.push_str(newline);
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_block_and_keeps_rest() {
        let original = "127.0.0.1 localhost\r\n";
        let once = apply(original, &["blog.test".into()]);
        assert!(once.starts_with("127.0.0.1 localhost\r\n\r\n# devlock-start"));
        assert!(once.contains("127.0.0.1 blog.test\r\n::1 blog.test"));

        let twice = apply(&once, &["shop.test".into()]);
        assert!(!twice.contains("blog.test"));
        assert!(twice.contains("shop.test"));
        assert_eq!(twice.matches("devlock-start").count(), 1);

        assert_eq!(apply(&twice, &[]), original);
    }
}
