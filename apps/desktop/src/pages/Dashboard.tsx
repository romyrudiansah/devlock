import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type Overview, type ServiceAction, type ServiceStatus } from "../api";
import LogModal from "./LogModal";

/** Gabungkan worker `php@8.3#0..3` menjadi satu baris `php@8.3`. */
function groupServices(services: ServiceStatus[]) {
  const groups = new Map<string, ServiceStatus[]>();
  for (const s of services) {
    groups.set(s.group, [...(groups.get(s.group) ?? []), s]);
  }
  return [...groups.entries()].map(([group, items]) => {
    const running = items.filter((s) => s.state === "running").length;
    const failed = items.find((s) => s.state === "failed");
    const state =
      running === items.length ? "running" : failed ? "failed" : running > 0 ? "partial" : items[0].state;
    const ports = items.map((s) => s.port).filter((p): p is number => p !== null);
    return {
      group,
      items,
      state,
      running,
      ports: ports.length > 1 ? `${Math.min(...ports)}–${Math.max(...ports)}` : (ports[0]?.toString() ?? "-"),
      restarts: items.reduce((sum, s) => sum + s.restarts, 0),
      uptime: Math.max(...items.map((s) => s.uptime_secs ?? -1)),
      error: failed?.error ?? items.find((s) => s.error)?.error ?? null,
    };
  });
}

function formatUptime(secs: number) {
  if (secs < 0) return "-";
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}j ${Math.floor((secs % 3600) / 60)}m`;
}

const STATE_LABEL: Record<string, string> = {
  running: "Berjalan",
  partial: "Sebagian",
  stopped: "Berhenti",
  restarting: "Restart…",
  failed: "Gagal",
};

export default function Dashboard() {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ ok: boolean; text: string } | null>(null);
  const [logFor, setLogFor] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setOverview(await api.overview());
    } catch (e) {
      setNotice({ ok: false, text: errorText(e) });
    }
  }, []);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 3000);
    const unlisten = listen<string | null>("services-changed", (event) => {
      if (event.payload) setNotice({ ok: false, text: event.payload });
      refresh();
    });
    return () => {
      clearInterval(timer);
      unlisten.then((off) => off());
    };
  }, [refresh]);

  const run = async (action: ServiceAction, target = "all") => {
    setBusy(`${action}:${target}`);
    setNotice(null);
    try {
      const result = await api.serviceAction(action, target);
      setOverview(result);
      if (result.message) setNotice({ ok: result.ok, text: result.message });
    } catch (e) {
      setNotice({ ok: false, text: errorText(e) });
    } finally {
      setBusy(null);
    }
  };

  const groups = useMemo(() => groupServices(overview?.services ?? []), [overview]);
  const runningCount = groups.filter((g) => g.state === "running").length;

  return (
    <section>
      <header className="page-header">
        <div>
          <h1>Service</h1>
          <p className="muted">
            {overview?.daemon_running
              ? `${runningCount} dari ${groups.length} service berjalan`
              : "Daemon belum berjalan"}
          </p>
        </div>
        <div className="actions">
          <button className="primary" disabled={!!busy} onClick={() => run("start")}>
            {busy === "start:all" ? "Menjalankan…" : "Start semua"}
          </button>
          <button disabled={!!busy || !overview?.daemon_running} onClick={() => run("reload")}>
            Reload
          </button>
          <button disabled={!!busy || !overview?.daemon_running} onClick={() => run("stop")}>
            Stop semua
          </button>
          <button className="ghost" onClick={() => setLogFor("daemon")}>
            Log daemon
          </button>
        </div>
      </header>

      {notice && <div className={`notice ${notice.ok ? "ok" : "error"}`}>{notice.text}</div>}
      {overview?.warnings.map((w) => (
        <div key={w} className="notice warn">
          {w}
        </div>
      ))}

      {overview && !overview.daemon_running ? (
        <div className="empty">
          <p>Belum ada service yang berjalan.</p>
          <p className="muted">
            Klik <strong>Start semua</strong> untuk menyalakan nginx, PHP, database, dan dev server
            sesuai project di <code>{overview.www}</code>.
          </p>
        </div>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Service</th>
              <th>Status</th>
              <th>Port</th>
              <th>Uptime</th>
              <th>Restart</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {groups.map((g) => (
              <tr key={g.group}>
                <td>
                  <div className="service-name">{g.group}</div>
                  {g.items.length > 1 && (
                    <div className="muted small">
                      {g.running}/{g.items.length} worker
                    </div>
                  )}
                  {g.error && <div className="error-text small">{g.error}</div>}
                </td>
                <td>
                  <span className={`badge ${g.state}`}>{STATE_LABEL[g.state] ?? g.state}</span>
                </td>
                <td className="mono">{g.ports}</td>
                <td>{formatUptime(g.uptime)}</td>
                <td>{g.restarts}</td>
                <td className="row-actions">
                  {g.state === "running" || g.state === "partial" ? (
                    <>
                      <button disabled={!!busy} onClick={() => run("restart", g.group)}>
                        Restart
                      </button>
                      <button disabled={!!busy} onClick={() => run("stop", g.group)}>
                        Stop
                      </button>
                    </>
                  ) : (
                    <button disabled={!!busy} onClick={() => run("start", g.group)}>
                      Start
                    </button>
                  )}
                  <button className="ghost" onClick={() => setLogFor(g.group)}>
                    Log
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
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
