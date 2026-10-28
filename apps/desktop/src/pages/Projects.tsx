import { useEffect, useState } from "react";
import { api, errorText, type Overview, type SiteInfo } from "../api";
import NewProject from "./NewProject";
import { t } from "../i18n";

export default function Projects() {
  const [sites, setSites] = useState<SiteInfo[] | null>(null);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

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
              <button className="primary" onClick={() => act(api.openUrl(site.url))}>
                {t("Buka")}
              </button>
              <button onClick={() => act(api.openFolder(site.dir))}>{t("Folder")}</button>
              <button onClick={() => act(api.openEditor(site.dir))}>Editor</button>
              <button onClick={() => act(api.openTerminal(site.dir))}>Terminal</button>
              {site.database && (
                <button
                  title={t("Buat database untuk {site} di {db}", { site: site.name, db: site.database })}
                  onClick={() =>
                    api
                      .createSiteDatabase(site.name)
                      .then(setNotice)
                      .catch((e) => setError(errorText(e)))
                  }
                >
                  {t("Buat DB")}
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
