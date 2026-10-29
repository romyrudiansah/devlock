import { useMemo, useState } from "react";
import LogModal from "./LogModal";
import { formatUptime, groupServices, isOn, Notices, stateLabel, Toggle, useOverview } from "./serviceKit";
import { Icon } from "../icons";
import { t } from "../i18n";

export default function Dashboard() {
  const { overview, busy, notice, setNotice, refresh, run } = useOverview();
  const [logFor, setLogFor] = useState<string | null>(null);

  const groups = useMemo(() => groupServices(overview?.services ?? []), [overview]);
  const runningCount = groups.filter((g) => g.state === "running").length;

  return (
    <section>
      <header className="page-header">
        <div>
          <h1>{t("Service")}</h1>
          <p className="muted">
            {overview?.daemon_running
              ? t("{running} dari {total} service berjalan", { running: runningCount, total: groups.length })
              : t("Daemon belum berjalan")}
          </p>
        </div>
        <div className="actions">
          <button className="primary" disabled={!!busy} onClick={() => run("start")}>
            <Icon name="play" size={16} />
            {busy === "start:all" ? t("Menjalankan…") : t("Start semua")}
          </button>
          <button disabled={!!busy || !overview?.daemon_running} onClick={() => run("reload")}>
            <Icon name="restart" size={16} />
            Reload
          </button>
          <button disabled={!!busy || !overview?.daemon_running} onClick={() => run("stop")}>
            <Icon name="stop" size={16} />
            {t("Stop semua")}
          </button>
          <button className="ghost" onClick={() => setLogFor("daemon")}>
            <Icon name="log" size={16} />
            {t("Log daemon")}
          </button>
        </div>
      </header>

      <Notices overview={overview} notice={notice} setNotice={setNotice} refresh={refresh} />

      {overview && !overview.daemon_running ? (
        <div className="empty">
          <p>{t("Belum ada service yang berjalan.")}</p>
          <p className="muted">
            {t("Klik")} <strong>{t("Start semua")}</strong>{" "}
            {t("untuk menyalakan nginx, PHP, database, dan dev server sesuai project di")}{" "}
            <code>{overview.www}</code>.
          </p>
        </div>
      ) : (
        <div className="list">
          <div className="list-row list-head">
            <span />
            <span>Service</span>
            <span>{t("Status")}</span>
            <span>Port</span>
            <span>Uptime</span>
            <span>Restart</span>
            <span />
          </div>
          {groups.map((g) => (
            <div key={g.group} className="list-row">
              <Toggle
                on={isOn(g)}
                disabled={!!busy}
                label={isOn(g) ? `Stop ${g.group}` : `Start ${g.group}`}
                onChange={(on) => run(on ? "start" : "stop", g.group)}
              />
              <div>
                <div className="service-name">{g.group}</div>
                {g.items.length > 1 && (
                  <div className="muted small">
                    {t("{running}/{total} worker", { running: g.running, total: g.items.length })}
                  </div>
                )}
                {g.error && <div className="error-text small">{g.error}</div>}
              </div>
              <span>
                <span className={`badge ${g.state}`}>{stateLabel(g.state)}</span>
              </span>
              <span className="mono">{g.ports}</span>
              <span>{formatUptime(g.uptime)}</span>
              <span>{g.restarts}</span>
              <span className="row-actions">
                {isOn(g) && (
                  <button className="icon-btn" title="Restart" aria-label={`Restart ${g.group}`} disabled={!!busy} onClick={() => run("restart", g.group)}>
                    <Icon name="restart" size={16} />
                  </button>
                )}
                <button className="icon-btn" title="Log" aria-label={`Log ${g.group}`} onClick={() => setLogFor(g.group)}>
                  <Icon name="log" size={16} />
                </button>
              </span>
            </div>
          ))}
        </div>
      )}

      {overview && (
        <footer className="muted small footer">
          Root: <code>{overview.root}</code> · HTTP port {overview.http_port} · TLD .{overview.tld}
        </footer>
      )}

      {logFor && <LogModal service={logFor} onClose={() => setLogFor(null)} />}
    </section>
  );
}
