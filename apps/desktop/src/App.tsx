import { useState } from "react";
import Dashboard from "./pages/Dashboard";
import Projects from "./pages/Projects";
import Runtimes from "./pages/Runtimes";

const TABS = [
  { id: "dashboard", label: "Service" },
  { id: "projects", label: "Project" },
  { id: "runtimes", label: "Runtime" },
] as const;

type Tab = (typeof TABS)[number]["id"];

export default function App() {
  const [tab, setTab] = useState<Tab>("dashboard");

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>
            ⬢
          </span>
          DevLock
        </div>
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`nav-item ${tab === t.id ? "active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </nav>
      <main className="content">
        {tab === "dashboard" && <Dashboard />}
        {tab === "projects" && <Projects />}
        {tab === "runtimes" && <Runtimes />}
      </main>
    </div>
  );
}
