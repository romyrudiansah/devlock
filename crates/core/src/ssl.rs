//! HTTPS lokal: CA DevLock (dibuat sekali, dipercaya di store Root milik user) dan
//! sertifikat per site yang ditandatangani CA itu.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};
use time::{Duration, OffsetDateTime};

use crate::paths::Paths;

/// Common Name CA; juga dipakai untuk mencari/menghapusnya di store Windows.
pub const CA_NAME: &str = "DevLock Local CA";

/// Masa berlaku sertifikat site. Browser menolak sertifikat leaf di atas 825 hari.
const SITE_VALIDITY_DAYS: i64 = 820;
const CA_VALIDITY_DAYS: i64 = 3650;

pub fn ca_cert_path(paths: &Paths) -> PathBuf {
    paths.ssl().join("devlock-ca.crt")
}

fn ca_key_path(paths: &Paths) -> PathBuf {
    paths.ssl().join("devlock-ca.key")
}

pub struct SiteCert {
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// Buat CA bila belum ada. Mengembalikan `true` bila baru dibuat.
pub fn ensure_ca(paths: &Paths) -> Result<bool> {
    let (cert, key) = (ca_cert_path(paths), ca_key_path(paths));
    if cert.is_file() && key.is_file() {
        return Ok(false);
    }
    std::fs::create_dir_all(paths.ssl())?;
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.distinguished_name = ca_name();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(CA_VALIDITY_DAYS);
    let key_pair = KeyPair::generate()?;
    let ca = params.self_signed(&key_pair)?;
    std::fs::write(&key, key_pair.serialize_pem())
        .with_context(|| format!("gagal menulis {}", key.display()))?;
    std::fs::write(&cert, ca.pem()).with_context(|| format!("gagal menulis {}", cert.display()))?;
    Ok(true)
}

fn ca_name() -> DistinguishedName {
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, CA_NAME);
    name.push(DnType::OrganizationName, "DevLock");
    name
}

/// Sertifikat untuk `domains` (domain utama + alias) di `ssl\sites\<name>.crt/.key`.
/// Dibuat ulang bila belum ada, daftar domain berubah, CA lebih baru, atau hampir kedaluwarsa.
pub fn site_cert(paths: &Paths, name: &str, domains: &[String]) -> Result<SiteCert> {
    ensure_ca(paths)?;
    let dir = paths.ssl().join("sites");
    let out = SiteCert {
        cert: dir.join(format!("{name}.crt")),
        key: dir.join(format!("{name}.key")),
    };
    let domains_file = dir.join(format!("{name}.domains"));
    let wanted = domains.join("\n");
    if !needs_renewal(&out, &domains_file, &wanted, &ca_cert_path(paths)) {
        return Ok(out);
    }

    std::fs::create_dir_all(&dir)?;
    let ca_key = KeyPair::from_pem(&std::fs::read_to_string(ca_key_path(paths))?)
        .context("kunci CA DevLock tidak valid")?;
    let ca_params =
        CertificateParams::from_ca_cert_pem(&std::fs::read_to_string(ca_cert_path(paths))?)
            .context("sertifikat CA DevLock tidak valid")?;
    // Subjek & kunci sama dengan CA tersimpan, jadi tanda tangan leaf terverifikasi oleh CA itu.
    let ca = ca_params.self_signed(&ca_key)?;

    let mut params = CertificateParams::new(domains.to_vec())?;
    let mut subject = DistinguishedName::new();
    subject.push(DnType::CommonName, domains[0].as_str());
    subject.push(DnType::OrganizationName, "DevLock");
    params.distinguished_name = subject;
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.use_authority_key_identifier_extension = true;
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(SITE_VALIDITY_DAYS);
    let key_pair = KeyPair::generate()?;
    let cert = params.signed_by(&key_pair, &ca, &ca_key)?;

    std::fs::write(&out.key, key_pair.serialize_pem())?;
    std::fs::write(&out.cert, cert.pem())?;
    std::fs::write(&domains_file, wanted)?;
    Ok(out)
}

fn needs_renewal(out: &SiteCert, domains_file: &Path, wanted: &str, ca: &Path) -> bool {
    if !out.cert.is_file() || !out.key.is_file() {
        return true;
    }
    if std::fs::read_to_string(domains_file).ok().as_deref() != Some(wanted) {
        return true;
    }
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (modified(&out.cert), modified(ca)) {
        // CA dibuat ulang setelah sertifikat site → tanda tangan lama tidak berlaku lagi.
        (Some(site), Some(ca)) if ca > site => true,
        // Perbarui sebulan sebelum kedaluwarsa.
        (Some(site), _) => site
            .elapsed()
            .is_ok_and(|age| age.as_secs() > (SITE_VALIDITY_DAYS as u64 - 30) * 86_400),
        _ => true,
    }
}

/// `true` bila CA DevLock ada di store Root milik user atau komputer.
pub fn is_trusted() -> bool {
    is_trusted_user() || is_trusted_machine()
}

/// CA DevLock di store Root milik user (Chrome, Edge, curl.exe).
pub fn is_trusted_user() -> bool {
    certutil(&["-user", "-store", "Root", CA_NAME]).is_ok_and(|out| out.contains(CA_NAME))
}

/// CA DevLock di store Root komputer (LocalMachine). Firefox hanya mengimpor root dari
/// store komputer, jadi ini yang membuat Firefox ikut percaya tanpa impor manual.
pub fn is_trusted_machine() -> bool {
    certutil(&["-store", "Root", CA_NAME]).is_ok_and(|out| out.contains(CA_NAME))
}

/// Pasang CA ke store Root komputer lewat UAC (untuk Firefox dan semua user di komputer ini).
pub fn trust_machine(paths: &Paths) -> Result<()> {
    ensure_ca(paths)?;
    let cert = ca_cert_path(paths);
    certutil_elevated(&["-addstore", "Root", &cert.to_string_lossy()])?;
    if !is_trusted_machine() {
        bail!("CA belum terpasang di store Root komputer");
    }
    Ok(())
}

/// Cabut CA DevLock dari store Root komputer (UAC).
pub fn untrust_machine() -> Result<()> {
    certutil_elevated(&["-delstore", "Root", CA_NAME])?;
    if is_trusted_machine() {
        bail!("CA masih ada di store Root komputer");
    }
    Ok(())
}

/// Skrip PowerShell yang menjalankan `certutil <args>` lewat `Start-Process -Verb RunAs`.
fn elevated_certutil_script(args: &[&str]) -> String {
    let quoted: Vec<String> = args
        .iter()
        // Kutip ganda di dalam string PowerShell agar path berspasi tetap satu argumen certutil.
        .map(|a| format!("'\"{}\"'", a.replace('\'', "''")))
        .collect();
    format!(
        "$p = Start-Process -FilePath certutil.exe -ArgumentList @({}) -Verb RunAs -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode",
        quoted.join(",")
    )
}

/// Jalankan certutil sebagai administrator (dialog UAC) dan tunggu hasilnya.
fn certutil_elevated(args: &[&str]) -> Result<()> {
    let script = elevated_certutil_script(args);
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::process::CREATE_NO_WINDOW);
    }
    let output = command.output().context("gagal menjalankan powershell")?;
    if !output.status.success() {
        bail!(
            "certutil {} gagal atau UAC dibatalkan: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Pasang CA ke store Root user. Windows menampilkan dialog konfirmasi ke pengguna.
pub fn trust(paths: &Paths) -> Result<()> {
    ensure_ca(paths)?;
    let cert = ca_cert_path(paths);
    certutil(&["-user", "-addstore", "Root", &cert.to_string_lossy()])
        .context("gagal memasang CA (dialog konfirmasi Windows ditolak?)")?;
    if !is_trusted_user() {
        bail!("CA belum terpasang di store Root (dialog konfirmasi Windows ditolak?)");
    }
    Ok(())
}

/// Cabut CA DevLock dari store Root user.
pub fn untrust() -> Result<()> {
    certutil(&["-user", "-delstore", "Root", CA_NAME]).map(|_| ())
}

fn certutil(args: &[&str]) -> Result<String> {
    let mut command = Command::new("certutil.exe");
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::process::CREATE_NO_WINDOW);
    }
    let output = command.output().context("gagal menjalankan certutil")?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        bail!("certutil {} gagal: {}", args.join(" "), text.trim());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use x509_parser::prelude::*;

    #[test]
    fn elevated_certutil_quotes_each_argument() {
        let script =
            elevated_certutil_script(&["-addstore", "Root", r"D:\Dev Lock\O'Brien\ca.crt"]);
        assert!(script.contains(
            r#"-ArgumentList @('"-addstore"','"Root"','"D:\Dev Lock\O''Brien\ca.crt"') -Verb RunAs"#
        ));
        assert!(script.ends_with("exit $p.ExitCode"));
    }

    #[test]
    fn site_cert_is_signed_by_ca() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        assert!(ensure_ca(&paths).unwrap());
        assert!(!ensure_ca(&paths).unwrap());

        let domains = vec!["blog.test".to_string(), "admin.blog.test".to_string()];
        let site = site_cert(&paths, "blog", &domains).unwrap();

        let ca_pem = std::fs::read(ca_cert_path(&paths)).unwrap();
        let (_, ca_pem) = parse_x509_pem(&ca_pem).unwrap();
        let ca = ca_pem.parse_x509().unwrap();
        let leaf_pem = std::fs::read(&site.cert).unwrap();
        let (_, leaf_pem) = parse_x509_pem(&leaf_pem).unwrap();
        let leaf = leaf_pem.parse_x509().unwrap();

        assert!(ca.is_ca());
        assert_eq!(leaf.issuer(), ca.subject());
        leaf.verify_signature(Some(ca.public_key()))
            .expect("tanda tangan leaf valid terhadap CA tersimpan");
        let san = leaf.subject_alternative_name().unwrap().unwrap();
        let names: Vec<String> = san
            .value
            .general_names
            .iter()
            .filter_map(|n| match n {
                GeneralName::DNSName(d) => Some(d.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(names, domains);
    }

    #[test]
    fn site_cert_reused_until_domains_change() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let first = site_cert(&paths, "shop", &["shop.test".into()]).unwrap();
        let pem = std::fs::read_to_string(&first.cert).unwrap();
        site_cert(&paths, "shop", &["shop.test".into()]).unwrap();
        assert_eq!(std::fs::read_to_string(&first.cert).unwrap(), pem);
        site_cert(&paths, "shop", &["shop.test".into(), "toko.test".into()]).unwrap();
        assert_ne!(std::fs::read_to_string(&first.cert).unwrap(), pem);
    }
}
