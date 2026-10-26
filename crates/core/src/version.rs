//! Perbandingan dan pencocokan versi sederhana (`8.3` cocok dengan `8.3.12`).

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(Vec<u64>);

impl Version {
    /// Parse `v20.11.1`, `8.3`, `11.4.3-winx64` → angka sebelum karakter non-numerik.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches(['v', 'V']);
        let mut parts = Vec::new();
        for part in text.split('.') {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            if digits.is_empty() {
                break;
            }
            parts.push(digits.parse().ok()?);
            if digits.len() != part.len() {
                break;
            }
        }
        (!parts.is_empty()).then_some(Self(parts))
    }

    pub fn parts(&self) -> &[u64] {
        &self.0
    }

    pub fn major(&self) -> u64 {
        self.0[0]
    }

    pub fn minor(&self) -> u64 {
        self.0.get(1).copied().unwrap_or(0)
    }

    /// `8.3.12` → `"8.3"`
    pub fn major_minor(&self) -> String {
        format!("{}.{}", self.major(), self.minor())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let len = self.0.len().max(other.0.len());
        (0..len)
            .map(|i| {
                let a = self.0.get(i).copied().unwrap_or(0);
                let b = other.0.get(i).copied().unwrap_or(0);
                a.cmp(&b)
            })
            .find(|o| o.is_ne())
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.0.iter().map(u64::to_string).collect();
        f.write_str(&parts.join("."))
    }
}

/// `true` bila spec kosong / `latest` / `*`, atau angka-angka spec adalah awalan versi.
pub fn matches(spec: &str, version: &str) -> bool {
    let spec = spec.trim();
    if spec.is_empty() || spec == "*" || spec.eq_ignore_ascii_case("latest") {
        return true;
    }
    match (Version::parse(spec), Version::parse(version)) {
        (Some(s), Some(v)) => v.0.starts_with(&s.0),
        _ => false,
    }
}

/// Versi tertinggi dari `candidates` yang cocok dengan `spec`.
pub fn pick<'a, I>(spec: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    candidates
        .into_iter()
        .filter(|v| matches(spec, v))
        .filter_map(|v| Version::parse(v).map(|parsed| (parsed, v)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v)
}

/// Urutkan string versi dari yang terbaru.
pub fn sort_desc(versions: &mut [String]) {
    versions.sort_by(|a, b| match (Version::parse(a), Version::parse(b)) {
        (Some(a), Some(b)) => b.cmp(&a),
        _ => b.cmp(a),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_variants() {
        assert_eq!(Version::parse("v20.11.1").unwrap().parts(), &[20, 11, 1]);
        assert_eq!(
            Version::parse("11.4.3-winx64").unwrap().parts(),
            &[11, 4, 3]
        );
        assert_eq!(Version::parse("8.3").unwrap().major_minor(), "8.3");
        assert!(Version::parse("lts/*").is_none());
    }

    #[test]
    fn prefix_matching() {
        assert!(matches("8.3", "8.3.12"));
        assert!(matches("8", "8.0.30"));
        assert!(!matches("8.3", "8.30.1"));
        assert!(!matches("8.3.1", "8.3.12"));
        assert!(matches("latest", "1.2.3"));
    }

    #[test]
    fn pick_highest() {
        let versions = ["8.2.24", "8.3.9", "8.3.12", "7.4.33"];
        assert_eq!(pick("8.3", versions), Some("8.3.12"));
        assert_eq!(pick("", versions), Some("8.3.12"));
        assert_eq!(pick("7", versions), Some("7.4.33"));
        assert_eq!(pick("5", versions), None);
    }

    #[test]
    fn sorting() {
        let mut v = vec!["1.9.0".to_string(), "1.10.0".into(), "1.2.0".into()];
        sort_desc(&mut v);
        assert_eq!(v, ["1.10.0", "1.9.0", "1.2.0"]);
    }
}
