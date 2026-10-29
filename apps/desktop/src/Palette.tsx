import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { api, errorText, type Overview, type SiteInfo } from "./api";
import { groupServices, isOn } from "./pages/serviceKit";
import { Icon, type IconName } from "./icons";
import { t } from "./i18n";

interface Command {
  id: string;
  icon: IconName;
  label: string;
  hint?: string;
  run: () => Promise<unknown> | void;
}

/** Palet perintah Ctrl+K: navigasi, start/stop service, dan buka site. */
export default function Palette({
  pages,
  onNavigate,
  onNewProject,
  onClose,
}: {
  pages: { id: string; label: string; icon: IconName }[];
  onNavigate: (tab: string) => void;
  onNewProject: () => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [sites, setSites] = useState<SiteInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    input.current?.focus();
    api.overview().then(setOverview).catch(() => {});
    api.listSites().then(setSites).catch(() => {});
  }, []);

  const commands = useMemo<Command[]>(() => {
    const service = (action: "start" | "stop" | "restart" | "reload", target = "all") => () =>
      api.serviceAction(action, target);
    const list: Command[] = [];
    const running = overview?.daemon_running ?? false;
    list.push(
      running
        ? { id: "stop-all", icon: "stop", label: t("Stop semua"), run: service("stop") }
        : { id: "start-all", icon: "play", label: t("Start semua"), run: service("start") },
    );
    if (running) {
      list.push({ id: "restart-all", icon: "restart", label: t("Restart semua"), run: service("restart") });
      list.push({ id: "reload", icon: "restart", label: t("Reload konfigurasi"), run: service("reload") });
    }
    for (const g of groupServices(overview?.services ?? [])) {
      list.push(
        isOn(g)
          ? { id: `stop:${g.group}`, icon: "stop", label: `Stop ${g.group}`, hint: "Service", run: service("stop", g.group) }
          : { id: `start:${g.group}`, icon: "play", label: `Start ${g.group}`, hint: "Service", run: service("start", g.group) },
      );
      if (isOn(g)) {
        list.push({ id: `restart:${g.group}`, icon: "restart", label: `Restart ${g.group}`, hint: "Service", run: service("restart", g.group) });
      }
    }
    for (const site of sites) {
      const domain = site.domains[0];
      list.push({ id: `open:${site.name}`, icon: "globe", label: t("Buka {site}", { site: domain }), hint: site.detail, run: () => api.openUrl(site.url) });
      list.push({ id: `editor:${site.name}`, icon: "code", label: t("Editor: {site}", { site: site.name }), hint: "Project", run: () => api.openEditor(site.dir) });
      list.push({ id: `term:${site.name}`, icon: "terminal", label: t("Terminal: {site}", { site: site.name }), hint: "Project", run: () => api.openTerminal(site.dir) });
      list.push({ id: `folder:${site.name}`, icon: "folder", label: t("Folder: {site}", { site: site.name }), hint: "Project", run: () => api.openFolder(site.dir) });
    }
    list.push({ id: "new-project", icon: "plus", label: t("Project baru"), run: onNewProject });
    list.push({ id: "install", icon: "runtimes", label: t("Install runtime…"), run: () => onNavigate("runtimes") });
    if (overview) {
      list.push({ id: "www", icon: "folder", label: t("Buka folder www"), run: () => api.openFolder(overview.www) });
    }
    for (const p of pages) {
      list.push({ id: `nav:${p.id}`, icon: p.icon, label: t("Ke {page}", { page: p.label }), hint: t("Halaman"), run: () => onNavigate(p.id) });
    }
    return list;
  }, [overview, sites, pages, onNavigate, onNewProject]);

  const filtered = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return commands.filter((c) => {
      const text = `${c.label} ${c.hint ?? ""}`.toLowerCase();
      return words.every((w) => text.includes(w));
    });
  }, [commands, query]);

  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    listRef.current?.children[active]?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const execute = async (c: Command | undefined) => {
    if (!c) return;
    try {
      await c.run();
      onClose();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") onClose();
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      execute(filtered[active]);
    }
  };

  return (
    <div className="modal-backdrop palette-backdrop" onClick={onClose}>
      <div className="palette" role="dialog" aria-label={t("Palet perintah")} onClick={(e) => e.stopPropagation()} onKeyDown={onKey}>
        <div className="palette-input">
          <Icon name="search" size={18} />
          <input
            ref={input}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("Cari perintah, service, atau site…")}
            aria-label={t("Cari perintah, service, atau site…")}
          />
          <kbd>Esc</kbd>
        </div>
        {error && <div className="notice error palette-error">{error}</div>}
        <ul ref={listRef} className="palette-list" role="listbox">
          {filtered.map((c, i) => (
            <li
              key={c.id}
              role="option"
              aria-selected={i === active}
              className={i === active ? "active" : ""}
              onMouseMove={() => setActive(i)}
              onClick={() => execute(c)}
            >
              <Icon name={c.icon} size={16} />
              <span className="palette-label">{c.label}</span>
              {c.hint && <span className="muted small">{c.hint}</span>}
            </li>
          ))}
          {filtered.length === 0 && <li className="muted palette-empty">{t("Tidak ada yang cocok.")}</li>}
        </ul>
      </div>
    </div>
  );
}
