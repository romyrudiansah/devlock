//! Metadata versi Windows (ProductName, FileVersion, ikon) agar exe bisa ditandatangani
//! dengan aturan metadata SignPath dan dikenali di Properties > Details.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "DevLock")
        .set("FileDescription", "DevLock CLI")
        .set("CompanyName", "DevLock")
        .set(
            "LegalCopyright",
            "Copyright (c) 2026 Romy Rudiansah. MIT OR Apache-2.0",
        )
        .set_icon("../../apps/desktop/src-tauri/icons/icon.ico");
    res.compile().expect("gagal menyematkan metadata Windows");
}
