import { useState } from "react";
import Dashboard from "./pages/Dashboard";
import Projects from "./pages/Projects";
import Runtimes from "./pages/Runtimes";
import Php from "./pages/Php";
import Settings from "./pages/Settings";
import { t } from "./i18n";
import logo from "./assets/logo.svg";

const TABS = [
  { id: "dashboard", label: "Service" },
  { id: "projects", label: "Project" },
  { id: "runtimes", label: "Runtime" },
  { id: "php", label: "PHP" },
  { id: "settings", label: "Settings" },
] as const;

type Tab = (typeof TABS)[number]["id"];

export default function App() {
  const [tab, setTab] = useState<Tab>("dashboard");

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="brand">
          <img className="brand-mark" src={logo} alt="" aria-hidden />
          DevLock
        </div>
        {TABS.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${tab === item.id ? "active" : ""}`}
            onClick={() => setTab(item.id)}
          >
            {t(item.label)}
          </button>
        ))}
      </nav>
      <main className="content">
        {tab === "dashboard" && <Dashboard />}
        {tab === "projects" && <Projects />}
        {tab === "runtimes" && <Runtimes />}
        {tab === "php" && <Php />}
        {tab === "settings" && <Settings />}
      </main>
    </div>
  );
}
