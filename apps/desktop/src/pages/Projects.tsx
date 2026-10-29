import { useEffect, useState } from "react";
import { api, errorText, type Overview, type SiteInfo } from "../api";
import NewProject from "./NewProject";
import { Icon } from "../icons";
import { t } from "../i18n";

export default function Projects({ startCreating = false }: { startCreating?: boolean }) {
  const [sites, setSites] = useState<SiteInfo[] | null>(null);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [creating, setCreating] = useState(startCreating);

  const refresh = async () => {
    setError(null);
    try {
      const [s, o] = await Promise.all([api.listSites(), api.overview()]);
      setSites(s);
      setOverview(o);
    } catch (e) {
      setError(errorText(e));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const act = (p: Promise<void>) => p.catch((e) => setError(errorText(e)));

  return (
    <section>
      <header className="page-header">
        <div>
          <h1>{t("Project")}</h1>
          <p className="muted">
            {t("Setiap folder di")} <code>{overview?.www ?? "www"}</code> {t("otomatis menjadi")}{" "}
            <code>
              {t("nama")}.{overview?.tld ?? "test"}
            </code>
          </p>
        </div>
        <div className="actions">
          <button className="primary" onClick={() => setCreating(true)}>
            {t("Project baru")}
          </button>
          {overview && <button onClick={() => act(api.openFolder(overview.www))}>{t("Buka folder www")}</button>}
          <button onClick={refresh}>{t("Muat ulang")}</button>
        </div>
      </header>

      {error && <div className="notice error">{error}</div>}
      {notice && <div className="notice ok">{notice}</div>}
      {overview && !overview.daemon_running && sites && sites.length > 0 && (
        <div className="notice warn">{t("Service belum berjalan — buka tab Service lalu klik Start semua.")}</div>
      )}

      {sites?.length === 0 && (
        <div className="empty">
          <p>{t("Belum ada project.")}</p>
          <p className="muted">
            {t("Buat folder baru di")} <code>{overview?.www}</code>
            {t(", lalu klik Muat ulang. Atur versi lewat file")} <code>.devlock.toml</code> {t("di folder project.")}
          </p>
        </div>
      )}

      <div className="cards">
        {sites?.map((site) => (
          <article key={site.name} className="card">
            <div className="card-head">
              <button className="link" onClick={() => act(api.openUrl(site.url))}>
                {site.domains[0]}
              </button>
              <span className={`badge kind-${site.kind}`}>{site.detail}</span>
            </div>
            {site.domains.length > 1 && (
              <div className="muted small">
                {t("alias")}: {site.domains.slice(1).join(", ")}
              </div>
            )}
            <dl className="meta">
              <dt>Database</dt>
              <dd>{site.database ?? "-"}</dd>
              <dt>Docroot</dt>
              <dd className="mono small">{site.docroot}</dd>
            </dl>
            {site.warning && <div className="error-text small">{site.warning}</div>}
            <div className="card-actions">
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
              {site.database && (
                <button
                  className="icon-btn"
                  title={t("Buat database untuk {site} di {db}", { site: site.name, db: site.database })}
                  aria-label={t("Buat DB")}
                  onClick={() =>
                    api
                      .createSiteDatabase(site.name)
                      .then(setNotice)
                      .catch((e) => setError(errorText(e)))
                  }
                >
                  <Icon name="database" size={16} />
                </button>
              )}
            </div>
          </article>
        ))}
      </div>
      {creating && <NewProject onClose={() => setCreating(false)} onCreated={refresh} />}
    </section>
  );
}
