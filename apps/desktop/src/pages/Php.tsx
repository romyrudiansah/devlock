import { useEffect, useMemo, useState } from "react";
import { api, errorText, type PhpInfo } from "../api";

const INI_LABELS: Record<string, string> = {
  memory_limit: "memory_limit",
  upload_max_filesize: "upload_max_filesize",
  post_max_size: "post_max_size",
  max_execution_time: "max_execution_time (detik)",
  display_errors: "display_errors (On/Off)",
  "date.timezone": "date.timezone (mis. Asia/Jakarta)",
};

export default function Php() {
  const [versions, setVersions] = useState<string[]>([]);
  const [version, setVersion] = useState("");
  const [info, setInfo] = useState<PhpInfo | null>(null);
  const [ini, setIni] = useState<Record<string, string>>({});
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    api
      .listRuntimes()
      .then((list) => {
        const php = list.find((r) => r.kind === "php");
        const installed = php?.installed ?? [];
        setVersions(installed);
        setVersion(php?.default_version ?? installed[0] ?? "");
      })
      .catch((e) => setNotice({ ok: false, text: errorText(e) }));
  }, []);

  const show = (next: PhpInfo) => {
    setInfo(next);
    setIni(Object.fromEntries(next.ini.map(([k, v]) => [k, v ?? ""])));
  };

  useEffect(() => {
    if (!version) return;
    setInfo(null);
    api
      .phpInfo(version)
      .then(show)
      .catch((e) => setNotice({ ok: false, text: errorText(e) }));
  }, [version]);

  const run = async (label: string, task: () => Promise<PhpInfo>, done: string) => {
    setBusy(label);
    setNotice(null);
    try {
      show(await task());
      setNotice({ ok: true, text: done });
    } catch (e) {
      setNotice({ ok: false, text: errorText(e) });
    } finally {
      setBusy(null);
    }
  };

  const extensions = useMemo(
    () => (info?.extensions ?? []).filter((e) => e.name.includes(filter.trim().toLowerCase())),
    [info, filter],
  );

  if (versions.length === 0) {
    return (
      <section>
        <header className="page-header">
          <div>
            <h1>PHP</h1>
            <p className="muted">Belum ada PHP terinstall. Install dulu lewat tab Runtime.</p>
          </div>
        </header>
        {notice && <div className={`notice ${notice.ok ? "ok" : "error"}`}>{notice.text}</div>}
      </section>
    );
  }

  return (
    <section>
      <header className="page-header">
        <div>
          <h1>PHP</h1>
          <p className="muted">Ekstensi, Xdebug, dan php.ini per versi. Pool PHP yang berjalan di-restart otomatis.</p>
        </div>
        <div className="actions">
          <select value={version} onChange={(e) => setVersion(e.target.value)} disabled={!!busy}>
            {versions.map((v) => (
              <option key={v} value={v}>
                PHP {v}
              </option>
            ))}
          </select>
          <button onClick={() => api.openPhpIni(version).catch((e) => setNotice({ ok: false, text: errorText(e) }))}>
            Buka php.ini
          </button>
        </div>
      </header>

      {notice && <div className={`notice ${notice.ok ? "ok" : "error"}`}>{notice.text}</div>}

      {info && (
        <div className="php-grid">
          <article className="runtime">
            <div className="runtime-head">
              <h2>Xdebug</h2>
              <span className={`badge ${info.xdebug_enabled ? "running" : ""}`}>
                {info.xdebug_enabled ? "aktif" : info.xdebug_installed ? "terpasang, nonaktif" : "belum terpasang"}
              </span>
            </div>
            <p className="muted small">
              Mode debug + develop, mulai saat ada trigger <code>XDEBUG_SESSION</code>, port 9003 (VS Code / PhpStorm).
            </p>
            <div className="card-actions">
              {!info.xdebug_installed ? (
                <button
                  className="primary"
                  disabled={!!busy}
                  onClick={() =>
                    run("xdebug", () => api.phpXdebug(version, "install"), "Xdebug terpasang dari xdebug.org dan aktif.")
                  }
                >
                  {busy === "xdebug" ? "Mengunduh…" : "Install & aktifkan"}
                </button>
              ) : (
                <button
                  disabled={!!busy}
                  onClick={() =>
                    run(
                      "xdebug",
                      () => api.phpXdebug(version, info.xdebug_enabled ? "off" : "on"),
                      info.xdebug_enabled ? "Xdebug dinonaktifkan." : "Xdebug diaktifkan.",
                    )
                  }
                >
                  {info.xdebug_enabled ? "Nonaktifkan" : "Aktifkan"}
                </button>
              )}
            </div>
          </article>

          <article className="runtime">
            <div className="runtime-head">
              <h2>php.ini</h2>
            </div>
            <form
              className="form"
              onSubmit={(e) => {
                e.preventDefault();
                run(
                  "ini",
                  () => api.phpSetIni(version, Object.entries(ini).filter(([, v]) => v.trim() !== "")),
                  "php.ini disimpan.",
                );
              }}
            >
              {info.ini.map(([key]) => (
                <label key={key} className="field">
                  <span className="small muted">{INI_LABELS[key] ?? key}</span>
                  <input
                    value={ini[key] ?? ""}
                    placeholder="default PHP"
                    onChange={(e) => setIni({ ...ini, [key]: e.target.value })}
                  />
                </label>
              ))}
              <div className="card-actions">
                <button className="primary" type="submit" disabled={!!busy}>
                  {busy === "ini" ? "Menyimpan…" : "Simpan"}
                </button>
              </div>
            </form>
          </article>

          <article className="runtime wide">
            <div className="runtime-head">
              <h2>Ekstensi</h2>
              <input
                className="search"
                placeholder="Cari ekstensi…"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              />
            </div>
            <div className="ext-list">
              {extensions.map((ext) => (
                <label key={ext.name} className="ext">
                  <input
                    type="checkbox"
                    checked={ext.enabled}
                    disabled={!!busy}
                    onChange={(e) =>
                      run(
                        `ext:${ext.name}`,
                        () => api.phpSetExtension(version, ext.name, e.target.checked),
                        `${ext.name} ${e.target.checked ? "diaktifkan" : "dinonaktifkan"}.`,
                      )
                    }
                  />
                  <span className="mono">{ext.name}</span>
                  {ext.zend && <span className="muted small">zend</span>}
                </label>
              ))}
            </div>
          </article>
        </div>
      )}
    </section>
  );
}
