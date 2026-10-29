import { useCallback, useEffect, useMemo, useState } from "react";
import Home from "./pages/Home";
import Dashboard from "./pages/Dashboard";
import Projects from "./pages/Projects";
import Runtimes from "./pages/Runtimes";
import Php from "./pages/Php";
import Settings from "./pages/Settings";
import Palette from "./Palette";
import { Icon, type IconName } from "./icons";
import { t } from "./i18n";

const TABS: { id: string; label: string; icon: IconName }[] = [
  { id: "home", label: "Beranda", icon: "home" },
  { id: "dashboard", label: "Service", icon: "services" },
  { id: "projects", label: "Project", icon: "projects" },
  { id: "runtimes", label: "Runtime", icon: "runtimes" },
  { id: "php", label: "PHP", icon: "php" },
];

const SETTINGS = { id: "settings", label: "Settings", icon: "settings" as IconName };

export default function App() {
  const [tab, setTab] = useState("home");
  const [palette, setPalette] = useState(false);
  // Dibuka dari palet: halaman Project langsung menampilkan form project baru.
  const [newProject, setNewProject] = useState(0);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((open) => !open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Diterjemahkan di sini, bukan saat modul dimuat: bahasa baru diatur sesudahnya di main.tsx.
  const pages = useMemo(() => [...TABS, SETTINGS].map((p) => ({ ...p, label: t(p.label) })), []);
  const navigate = useCallback((id: string) => {
    setNewProject(0);
    setTab(id);
  }, []);
  const startProject = useCallback(() => {
    setNewProject((n) => n + 1);
    setTab("projects");
  }, []);

  const railItem = (item: { id: string; label: string; icon: IconName }) => (
    <button
      key={item.id}
      className={`rail-item ${tab === item.id ? "active" : ""}`}
      aria-current={tab === item.id ? "page" : undefined}
      title={t(item.label)}
      onClick={() => navigate(item.id)}
    >
      <Icon name={item.icon} />
      <span>{t(item.label)}</span>
    </button>
  );

  return (
    <div className="app">
      <nav className="rail" aria-label={t("Navigasi")}>
        {TABS.map(railItem)}
        <span className="spacer" />
        <button className="rail-item" title={t("Palet perintah (Ctrl+K)")} onClick={() => setPalette(true)}>
          <Icon name="search" />
          <span>{t("Cari")}</span>
        </button>
        {railItem(SETTINGS)}
      </nav>
      <main className="content">
        {tab === "home" && <Home onNavigate={navigate} onPalette={() => setPalette(true)} />}
        {tab === "dashboard" && <Dashboard />}
        {tab === "projects" && <Projects key={newProject} startCreating={newProject > 0} />}
        {tab === "runtimes" && <Runtimes />}
        {tab === "php" && <Php />}
        {tab === "settings" && <Settings />}
      </main>
      {palette && (
        <Palette pages={pages} onNavigate={navigate} onNewProject={startProject} onClose={() => setPalette(false)} />
      )}
    </div>
  );
}
