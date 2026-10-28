//! Mirror unduhan runtime: bila sumber asli gagal, arsip dicoba dari mirror. Checksum tetap
//! dari sumber asli/katalog resmi, jadi mirror tidak bisa menyelipkan isi lain.

use crate::config::Config;

/// Mirror bawaan: awalan URL asli → awalan mirror.
pub const BUILTIN: &[(&str, &str)] = &[
    // Situs PHP for Windows resmi yang lain (isi sama dengan downloads.php.net).
    (
        "https://downloads.php.net/~windows/releases/",
        "https://windows.php.net/downloads/releases/",
    ),
    // Mirror Node.js populer (npmmirror, Alibaba).
    (
        "https://nodejs.org/dist/",
        "https://npmmirror.com/mirrors/node/",
    ),
];

/// URL yang dicoba berurutan: asli dulu, lalu mirror dari `[mirrors]`, lalu mirror bawaan
/// (bila `[download] builtin_mirrors` tidak dimatikan). Tanpa duplikat.
pub fn candidates(url: &str, config: &Config) -> Vec<String> {
    let mut out = vec![url.to_string()];
    let custom = config.mirrors.iter().flat_map(|(origin, mirrors)| {
        mirrors
            .iter()
            .map(move |mirror| (origin.as_str(), mirror.as_str()))
    });
    let builtin = BUILTIN
        .iter()
        .copied()
        .filter(|_| config.download.builtin_mirrors);
    for (origin, mirror) in custom.chain(builtin) {
        if let Some(rest) = url.strip_prefix(origin) {
            let candidate = format!("{}{rest}", ensure_slash(origin, mirror));
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

/// Samakan akhiran `/` mirror dengan awalan asli agar sambungan path tetap benar.
fn ensure_slash<'a>(origin: &str, mirror: &'a str) -> std::borrow::Cow<'a, str> {
    if origin.ends_with('/') && !mirror.ends_with('/') {
        format!("{mirror}/").into()
    } else {
        mirror.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_first_then_custom_then_builtin() {
        let mut config = Config::default();
        let url = "https://nodejs.org/dist/v24.1.0/node-v24.1.0-win-x64.zip";
        assert_eq!(
            candidates(url, &config),
            [
                url,
                "https://npmmirror.com/mirrors/node/v24.1.0/node-v24.1.0-win-x64.zip"
            ]
        );

        config.mirrors.insert(
            "https://nodejs.org/dist/".into(),
            vec!["https://mirror.kantor.local/node".into()],
        );
        let list = candidates(url, &config);
        assert_eq!(list.len(), 3);
        assert_eq!(
            list[1],
            "https://mirror.kantor.local/node/v24.1.0/node-v24.1.0-win-x64.zip"
        );

        config.download.builtin_mirrors = false;
        assert_eq!(candidates(url, &config).len(), 2);
        assert_eq!(
            candidates("https://github.com/x/y.zip", &config),
            ["https://github.com/x/y.zip"]
        );
    }
}
