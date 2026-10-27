import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type AvailableRelease, type InstallProgress, type RuntimeInfo } from "../api";

const LABELS: Record<string, string> = {
  php: "PHP",
  node: "Node.js",
  composer: "Composer",
  nginx: "nginx",
  apache: "Apache",
  mysql: "MySQL",
  mariadb: "MariaDB",
  postgresql: "PostgreSQL",
};

function formatBytes(n: number) {
  return n > 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.round(n / 1024)} KB`;
}

export default function Runtimes() {
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);

  const refresh = () =>
    api
      .listRuntimes()
      .then(setRuntimes)
      .catch((e) => setError(errorText(e)));

  useEffect(() => {
    refresh();
    const unlisten = listen<InstallProgress>("install-progress", (e) => setProgress(e.payload));
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  const install = async (kind: string, version: string) => {
    setError(null);
    setInstalling(kind);
    setProgress(null);
    try {
      await api.installRuntime(kind, version);
      await refresh();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setInstalling(null);
      setProgress(null);
    }
  };

  const uninstall = async (kind: string, version: string) => {
    if (!confirm(`Hapus ${LABELS[kind] ?? kind} ${version}?`)) return;
    setError(null);
    try {
      await api.uninstallRuntime(kind, version);
      await refresh();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const setDefault = async (kind: string, version: string) => {
    setError(null);
    try {
      await api.setDefaultRuntime(kind, version);
      await refresh();
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <section>
      <header className="page-header">
        <div>
          <h1>Runtime</h1>
          <p className="muted">Banyak versi bisa terinstall sekaligus; tiap project memilih versinya sendiri.</p>
        </div>
      </header>

      {error && <div className="notice error">{error}</div>}

      <div className="runtime-list">
        {runtimes.map((rt) => (
          <RuntimeRow
            key={rt.kind}
            runtime={rt}
            busy={installing !== null}
            progress={installing === rt.kind ? progress : null}
            installing={installing === rt.kind}
            onInstall={(v) => install(rt.kind, v)}
            onUninstall={(v) => uninstall(rt.kind, v)}
            onSetDefault={(v) => setDefault(rt.kind, v)}
          />
        ))}
      </div>
    </section>
  );
}

function RuntimeRow(props: {
  runtime: RuntimeInfo;
  busy: boolean;
  installing: boolean;
  progress: InstallProgress | null;
  onInstall: (version: string) => void;
  onUninstall: (version: string) => void;
  onSetDefault: (version: string) => void;
}) {
  const { runtime: rt, progress } = props;
  const [available, setAvailable] = useState<AvailableRelease[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [choice, setChoice] = useState("");
  const [error, setError] = useState<string | null>(null);
  const isDatabase = ["mysql", "mariadb", "postgresql"].includes(rt.kind);
  const isWebserver = rt.kind === "nginx" || rt.kind === "apache";
  const canDefault = !isDatabase && !isWebserver;

  const loadAvailable = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await api.availableRuntimes(rt.kind);
      setAvailable(list);
      setChoice(list[0]?.version ?? "");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  };

  const percent =
    progress?.stage === "download" && progress.total ? Math.round((progress.downloaded / progress.total) * 100) : null;

  return (
    <article className="runtime">
      <div className="runtime-head">
        <h2>{LABELS[rt.kind] ?? rt.kind}</h2>
        {available === null ? (
          <button disabled={loading || props.busy} onClick={loadAvailable}>
            {loading ? "Memuat daftar…" : "Install versi…"}
          </button>
        ) : (
          <div className="install-form">
            <select value={choice} onChange={(e) => setChoice(e.target.value)} disabled={props.busy}>
              {available.map((r) => (
                <option key={r.version} value={r.version}>
                  {r.version}
                  {r.lts ? (isWebserver ? " (stable)" : " (LTS)") : ""}
                  {rt.installed.includes(r.version) ? " — terinstall" : ""}
                </option>
              ))}
            </select>
            <button
              className="primary"
              disabled={props.busy || !choice || rt.installed.includes(choice)}
              onClick={() => props.onInstall(choice)}
            >
              {props.installing ? "Menginstall…" : "Install"}
            </button>
            <button className="ghost" disabled={props.busy} onClick={() => setAvailable(null)}>
              Batal
            </button>
          </div>
        )}
      </div>

      {props.installing && progress && (
        <div className="progress">
          <div className="progress-bar" style={{ width: `${percent ?? 100}%` }} />
          <span className="small">
            {progress.version} · {progress.stage}
            {percent !== null &&
              ` ${percent}% (${formatBytes(progress.downloaded)}${progress.total ? ` / ${formatBytes(progress.total)}` : ""})`}
          </span>
        </div>
      )}
      {error && <div className="error-text small">{error}</div>}

      {rt.installed.length === 0 ? (
        <p className="muted small">Belum ada versi terinstall.</p>
      ) : (
        <ul className="versions">
          {rt.installed.map((v) => (
            <li key={v}>
              <span className="mono">{v}</span>
              {rt.default_version === v && <span className="badge running">default</span>}
              <span className="spacer" />
              {canDefault && rt.default_version !== v && (
                <button className="ghost small" onClick={() => props.onSetDefault(v)}>
                  Jadikan default
                </button>
              )}
              <button className="ghost small danger" disabled={props.busy} onClick={() => props.onUninstall(v)}>
                Hapus
              </button>
            </li>
          ))}
        </ul>
      )}
    </article>
  );
}
