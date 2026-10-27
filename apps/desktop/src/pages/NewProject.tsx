import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type CreatedProject, type RuntimeInfo, type TemplateInfo } from "../api";
import { Version } from "./version";

/// Instance database dari versi terinstall: MySQL/MariaDB per major.minor, PostgreSQL per major.
function instances(runtimes: RuntimeInfo[]): string[] {
  const out = new Set<string>();
  for (const rt of runtimes) {
    if (!["mysql", "mariadb", "postgresql"].includes(rt.kind)) continue;
    for (const v of rt.installed) {
      const parts = Version.parts(v);
      out.add(`${rt.kind}@${rt.kind === "postgresql" ? parts[0] : `${parts[0]}.${parts[1] ?? 0}`}`);
    }
  }
  return [...out];
}

export default function NewProject({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [template, setTemplate] = useState("laravel");
  const [name, setName] = useState("");
  const [php, setPhp] = useState("");
  const [node, setNode] = useState("");
  const [database, setDatabase] = useState("");
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<CreatedProject | null>(null);
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    api.listTemplates().then(setTemplates).catch((e) => setError(errorText(e)));
    api
      .listRuntimes()
      .then((list) => {
        setRuntimes(list);
        const dbs = instances(list);
        setDatabase(dbs[0] ?? "");
      })
      .catch((e) => setError(errorText(e)));
    const unlisten = listen<string>("scaffold-log", (e) => setLog((l) => [...l.slice(-400), e.payload]));
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [log]);

  const info = templates.find((t) => t.id === template);
  const phpVersions = runtimes.find((r) => r.kind === "php")?.installed ?? [];
  const nodeVersions = runtimes.find((r) => r.kind === "node")?.installed ?? [];
  const dbInstances = useMemo(() => instances(runtimes), [runtimes]);
  const validName = /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(name);
  const missingRuntime =
    (info?.uses_php && phpVersions.length === 0 && "PHP") ||
    (info?.uses_node && nodeVersions.length === 0 && "Node.js") ||
    (info?.needs_database && dbInstances.length === 0 && "database (MySQL/MariaDB/PostgreSQL)");

  const submit = async () => {
    setBusy(true);
    setError(null);
    setLog([]);
    try {
      const result = await api.createProject({
        template,
        name,
        php: info?.uses_php ? php : undefined,
        node: info?.uses_node ? node : undefined,
        database: info?.needs_database || database ? database : undefined,
      });
      setCreated(result);
      onCreated();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={busy ? undefined : onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <h2>Project baru</h2>
          <button className="ghost" onClick={onClose} disabled={busy}>
            Tutup
          </button>
        </header>

        <div className="modal-body">
          {created ? (
            <div className="form">
              <div className="notice ok">Project siap di {created.dir}</div>
              <p>
                URL:{" "}
                <button className="link" onClick={() => api.openUrl(created.url)}>
                  {created.url}
                </button>
              </p>
              {created.database && <p className="muted">Database: {created.database}</p>}
              <ul className="small">
                {created.notes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            </div>
          ) : (
            <div className="form">
              <label className="field">
                <span className="small muted">Template</span>
                <select value={template} onChange={(e) => setTemplate(e.target.value)} disabled={busy}>
                  {templates.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span className="small muted">Nama (folder &amp; domain): huruf kecil, angka, tanda minus</span>
                <input
                  value={name}
                  placeholder="mis. toko-online"
                  onChange={(e) => setName(e.target.value.toLowerCase())}
                  disabled={busy}
                />
              </label>
              {info?.uses_php && (
                <label className="field">
                  <span className="small muted">Versi PHP</span>
                  <select value={php} onChange={(e) => setPhp(e.target.value)} disabled={busy}>
                    <option value="">Default</option>
                    {phpVersions.map((v) => (
                      <option key={v} value={v}>
                        PHP {v}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {info?.uses_node && (
                <label className="field">
                  <span className="small muted">Versi Node.js</span>
                  <select value={node} onChange={(e) => setNode(e.target.value)} disabled={busy}>
                    <option value="">Default</option>
                    {nodeVersions.map((v) => (
                      <option key={v} value={v}>
                        Node {v}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {(info?.uses_php || info?.needs_database) && (
                <label className="field">
                  <span className="small muted">Database {info?.needs_database ? "(wajib)" : "(opsional)"}</span>
                  <select value={database} onChange={(e) => setDatabase(e.target.value)} disabled={busy}>
                    {!info?.needs_database && <option value="">Tanpa database</option>}
                    {dbInstances.map((d) => (
                      <option key={d} value={d}>
                        {d}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {missingRuntime && <div className="notice warn">Install {missingRuntime} dulu lewat tab Runtime.</div>}
              {name && !validName && <div className="notice warn">Nama hanya huruf kecil, angka, dan tanda minus.</div>}
              {error && <div className="notice error">{error}</div>}
              <div className="card-actions">
                <button className="primary" disabled={busy || !validName || !!missingRuntime} onClick={submit}>
                  {busy ? "Membuat…" : "Buat project"}
                </button>
              </div>
            </div>
          )}
          {log.length > 0 && (
            <pre ref={logRef} className="log short">
              {log.join("\n")}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}
