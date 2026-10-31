import { useEffect, useMemo, useState } from "react";
import { api, errorText, type SiteInfo } from "../api";
import { groupServices, isOn, Notices, stateLabel, Toggle, useOverview } from "./serviceKit";
import { Icon } from "../icons";
import { t } from "../i18n";
import logo from "../assets/logo.svg";

/** Beranda ringkas: status keseluruhan, satu tombol Start/Stop, tile service, dan site. */
export default function Home({ onNavigate, onPalette }: { onNavigate: (tab: string) => void; onPalette: () => void }) {
  const { overview, busy, notice, setNotice, refresh, run } = useOverview();
  const [sites, setSites] = useState<SiteInfo[] | null>(null);

  useEffect(() => {
    api
      .listSites()
      .then(setSites)
      .catch(() => setSites([]));
  }, []);

  const groups = useMemo(() => groupServices(overview?.services ?? []), [overview]);
  const running = overview?.daemon_running ?? false;
  const on = groups.filter(isOn).length;
  const failed = groups.filter((g) => g.state === "failed").length;
  const act = (p: Promise<void>) => p.catch((e) => setNotice({ ok: false, text: errorText(e) }));

  const title = !overview
    ? t("Memuat…")
    : !running
      ? t("DevLock berhenti")
      : failed > 0
        ? t("{n} service bermasalah", { n: failed })
        : t("Semua berjalan");
  const subtitle = running
    ? t("{on} service · {sites} site · http://*.{tld}", { on, sites: sites?.length ?? 0, tld: overview?.tld ?? "test" })
    : t("Nyalakan untuk menjalankan web server, PHP, database, dan dev server project Anda.");

  return (
    <section className="home">
      <header className="hero">
        <img src={logo} alt="" aria-hidden className="hero-logo" />
        <div className="hero-text">
          <h1>
            <span className={`dot ${!running ? "off" : failed > 0 ? "bad" : "good"}`} />
            {title}
          </h1>
          <p className="muted">{subtitle}</p>
        </div>
        <div className="actions">
          {running && (
            <button disabled={!!busy} onClick={() => run("restart")}>
              <Icon name="restart" size={16} />
              Restart
            </button>
          )}
          {running ? (
            <button className="primary big" disabled={!!busy} onClick={() => run("stop")}>
              <Icon name="stop" size={16} />
              {t("Stop semua")}
            </button>
          ) : (
            <button className="primary big" disabled={!!busy} onClick={() => run("start")}>
              <Icon name="play" size={16} />
              {busy === "start:all" ? t("Menjalankan…") : t("Start semua")}
            </button>
          )}
        </div>
      </header>

      <button className="palette-bar" onClick={onPalette}>
        <Icon name="search" size={16} />
        <span>{t("Cari perintah, service, atau site…")}</span>
        <kbd>Ctrl</kbd>
        <kbd>K</kbd>
      </button>

      <Notices overview={overview} notice={notice} setNotice={setNotice} refresh={refresh} />

      <div className="section-head">
        <h2>{t("Service")}</h2>
        <button className="link-btn" onClick={() => onNavigate("dashboard")}>
          {t("Detail")} <Icon name="arrow" size={14} />
        </button>
      </div>
      {groups.length === 0 ? (
        <p className="muted small">
          {running ? t("Belum ada service untuk project saat ini.") : t("Service muncul di sini setelah DevLock dinyalakan.")}
        </p>
      ) : (
        <div className="tiles">
          {groups.map((g) => (
            <div key={g.group} className={`tile ${g.state}`}>
              <div className="tile-head">
                <span className={`dot ${g.state === "running" ? "good" : g.state === "failed" ? "bad" : isOn(g) ? "warn" : "off"}`} />
                <span className="tile-name">{g.group}</span>
                <Toggle
                  on={isOn(g)}
                  disabled={!!busy}
                  label={isOn(g) ? `Stop ${g.group}` : `Start ${g.group}`}
                  onChange={(next) => run(next ? "start" : "stop", g.group)}
                />
              </div>
              <div className="tile-meta">
                {stateLabel(g.state)}
                {g.ports !== "-" && <> · :{g.ports}</>}
                {g.items.length > 1 && <> · {t("{running}/{total} worker", { running: g.running, total: g.items.length })}</>}
              </div>
              {g.error && <div className="error-text small">{g.error}</div>}
            </div>
          ))}
        </div>
      )}

      <div className="section-head">
        <h2>{t("Project")}</h2>
        <button className="link-btn" onClick={() => onNavigate("projects")}>
          {t("Kelola")} <Icon name="arrow" size={14} />
        </button>
      </div>
      {sites?.length === 0 ? (
        <p className="muted small">
          {t("Belum ada project. Buat folder di")} <code>{overview?.www ?? "www"}</code> {t("atau klik Project baru.")}
        </p>
      ) : (
        <div className="list">
          {sites?.map((site) => (
            <div key={site.name} className="site-row">
              <Icon name="globe" size={18} />
              <button className="site-domain" onClick={() => act(api.openUrl(site.url))}>
                {site.domains[0]}
              </button>
              <span className="muted small site-detail">
                {site.group && <>{site.group} · </>}
                {site.detail}
                {site.database && <> · {site.database}</>}
              </span>
              {site.warning && (
                <span className="badge failed" title={site.warning}>
                  !
                </span>
              )}
              <span className="row-actions">
                <button className="icon-btn" title={t("Buka")} aria-label={t("Buka")} onClick={() => act(api.openUrl(site.url))}>
                  <Icon name="globe" size={16} />
                </button>
                <button className="icon-btn" title={t("Folder")} aria-label={t("Folder")} onClick={() => act(api.openFolder(site.dir))}>
                  <Icon name="folder" size={16} />
                </button>
                <button className="icon-btn" title="Terminal" aria-label="Terminal" onClick={() => act(api.openTerminal(site.dir))}>
                  <Icon name="terminal" size={16} />
                </button>
                <button className="icon-btn" title="Editor" aria-label="Editor" onClick={() => act(api.openEditor(site.dir))}>
                  <Icon name="code" size={16} />
                </button>
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
