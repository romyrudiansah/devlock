import { useEffect, useState } from "react";
import { api, errorText, type Settings as SettingsData } from "../api";
import { getLang, t } from "../i18n";

export default function Settings() {
  const [form, setForm] = useState<SettingsData | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then(setForm)
      .catch((e) => setNotice({ ok: false, text: errorText(e) }));
  }, []);

  if (!form) {
    return (
      <section>
        <header className="page-header">
          <h1>Settings</h1>
        </header>
        {notice && <div className="notice error">{notice.text}</div>}
      </section>
    );
  }

  const set = <K extends keyof SettingsData>(key: K, value: SettingsData[K]) => setForm({ ...form, [key]: value });
  const num = (value: string) => Math.max(0, Math.min(65535, Number(value) || 0));

  const save = async () => {
    setBusy(true);
    setNotice(null);
    try {
      const message = await api.saveSettings(form);
      // Bahasa diterapkan saat UI dimuat ulang.
      if (form.language !== getLang()) {
        window.location.reload();
        return;
      }
      setNotice({ ok: true, text: message });
    } catch (e) {
      setNotice({ ok: false, text: errorText(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section>
      <header className="page-header">
        <div>
          <h1>Settings</h1>
          <p className="muted">
            {t("Disimpan ke devlock.toml (komentar tetap utuh) dan langsung diterapkan bila daemon berjalan.")}
          </p>
        </div>
        <div className="actions">
          <button className="primary" disabled={busy} onClick={save}>
            {busy ? t("Menyimpan…") : t("Simpan")}
          </button>
        </div>
      </header>

      {notice && <div className={`notice ${notice.ok ? "ok" : "error"}`}>{notice.text}</div>}

      <div className="php-grid">
        <article className="runtime">
          <h2>{t("Project")}</h2>
          <div className="form">
            <label className="field">
              <span className="small muted">{t("Folder www (tiap subfolder menjadi site)")}</span>
              <input value={form.www} onChange={(e) => set("www", e.target.value)} />
            </label>
            <label className="field">
              <span className="small muted">{t("TLD domain lokal")}</span>
              <input value={form.tld} onChange={(e) => set("tld", e.target.value)} />
            </label>
          </div>
        </article>

        <article className="runtime">
          <h2>{t("Web server")}</h2>
          <div className="form">
            <label className="field">
              <span className="small muted">{t("Port HTTP")}</span>
              <input type="number" value={form.http_port} onChange={(e) => set("http_port", num(e.target.value))} />
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={form.https_enabled}
                onChange={(e) => set("https_enabled", e.target.checked)}
              />
              {t("HTTPS untuk semua site (butuh")} <code>devlock ssl trust</code> {t("sekali)")}
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={form.https_redirect}
                onChange={(e) => set("https_redirect", e.target.checked)}
              />
              {t("Alihkan HTTP ke HTTPS untuk site ber-HTTPS (per project:")} <code>https_redirect</code>)
            </label>
            <label className="field">
              <span className="small muted">{t("Port HTTPS")}</span>
              <input type="number" value={form.https_port} onChange={(e) => set("https_port", num(e.target.value))} />
            </label>
            <label className="field">
              <span className="small muted">{t("Worker php-cgi per versi PHP (1–10)")}</span>
              <input
                type="number"
                min={1}
                max={10}
                value={form.php_workers}
                onChange={(e) => set("php_workers", Math.max(1, Math.min(10, Number(e.target.value) || 1)))}
              />
            </label>
          </div>
        </article>

        <article className="runtime">
          <h2>Autostart</h2>
          <div className="form">
            <label className="check">
              <input
                type="checkbox"
                checked={form.autostart_services}
                onChange={(e) => set("autostart_services", e.target.checked)}
              />
              {t("Jalankan semua service saat DevLock dibuka")}
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={form.start_with_windows}
                onChange={(e) => set("start_with_windows", e.target.checked)}
              />
              {t("Buka DevLock (di tray) saat login Windows")}
            </label>
            <label className="field">
              <span className="small muted">
                {t("Editor (kosong = deteksi VS Code, Cursor, Windsurf, Zed, Sublime, PhpStorm)")}
              </span>
              <input
                value={form.editor}
                placeholder="code"
                onChange={(e) => set("editor", e.target.value)}
              />
            </label>
          </div>
        </article>

        <article className="runtime">
          <h2>{t("Tampilan")}</h2>
          <div className="form">
            <label className="field">
              <span className="small muted">{t("Bahasa")}</span>
              <select value={form.language} onChange={(e) => set("language", e.target.value)}>
                <option value="id">Bahasa Indonesia</option>
                <option value="en">English</option>
              </select>
            </label>
            <p className="muted small">
              {t("Bahasa berubah setelah disimpan (jendela dimuat ulang). Pesan dari CLI tetap berbahasa Indonesia.")}
            </p>
          </div>
        </article>
      </div>
    </section>
  );
}
