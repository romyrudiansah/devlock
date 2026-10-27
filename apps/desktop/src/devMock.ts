// Pratinjau UI di browser biasa (`npm run dev` tanpa Tauri) dengan data contoh.
// Hanya dimuat di mode development ketika backend Tauri tidak tersedia.

import type { AvailableRelease, Overview, RuntimeInfo, ServiceStatus, SiteInfo } from "./api";

const service = (name: string, group: string, port: number, state: ServiceStatus["state"] = "running"): ServiceStatus => ({
  name,
  group,
  state,
  pid: state === "running" ? 10000 + port : null,
  port,
  uptime_secs: state === "running" ? 754 : null,
  restarts: name === "php@8.3#0" ? 1 : 0,
  error: state === "failed" ? "port 3384 sudah dipakai proses lain" : null,
  log: `D:\\DevLock\\logs\\${group}.log`,
});

let running = true;

const services = (): ServiceStatus[] => [
  ...[0, 1, 2, 3].map((i) => service(`php@7.4#${i}`, "php@7.4", 9740 + i)),
  ...[0, 1, 2, 3].map((i) => service(`php@8.3#${i}`, "php@8.3", 9830 + i)),
  service("nginx", "nginx", 80),
  service("mariadb@11.4", "mariadb@11.4", 3424),
  service("postgresql@18", "postgresql@18", 5418),
  service("mysql@8.4", "mysql@8.4", 3384, "failed"),
  service("dev:dash", "dev:dash", 3999),
];

const overview = (message: string | null = null): Overview => ({
  root: "D:\\DevLock",
  www: "D:\\DevLock\\www",
  tld: "test",
  http_port: 80,
  daemon_running: running,
  services: running ? services() : [],
  warnings: running ? ["mysql@8.4 belum terinstall; jalankan `devlock install mysql 8.4`"] : [],
  message,
  ok: true,
});

const sites: SiteInfo[] = [
  {
    name: "dash",
    url: "http://dash.test",
    domains: ["dash.test", "admin.test"],
    kind: "proxy",
    detail: "dev server :3999",
    database: null,
    dir: "D:\\DevLock\\www\\dash",
    docroot: "D:\\DevLock\\www\\dash",
    warning: null,
  },
  {
    name: "legacy",
    url: "http://legacy.test",
    domains: ["legacy.test"],
    kind: "php",
    detail: "PHP 7.4",
    database: "mariadb@11.4",
    dir: "D:\\DevLock\\www\\legacy",
    docroot: "D:\\DevLock\\www\\legacy",
    warning: null,
  },
  {
    name: "shop",
    url: "http://shop.test",
    domains: ["shop.test"],
    kind: "static",
    detail: "Static",
    database: "postgresql@18",
    dir: "D:\\DevLock\\www\\shop",
    docroot: "D:\\DevLock\\www\\shop\\public",
    warning: "php 8.2 (dari D:\\DevLock\\www\\shop\\.devlock.toml) belum terinstall; jalankan `devlock install php 8.2`",
  },
];

const runtimes: RuntimeInfo[] = [
  { kind: "php", installed: ["8.3.36", "7.4.33"], default_spec: "8.3.36", default_version: "8.3.36" },
  { kind: "node", installed: ["22.23.4", "20.20.2"], default_spec: "22.23.4", default_version: "22.23.4" },
  { kind: "composer", installed: ["2.10.3"], default_spec: "2.10.3", default_version: "2.10.3" },
  { kind: "nginx", installed: ["1.28.3"], default_spec: null, default_version: null },
  { kind: "mysql", installed: [], default_spec: null, default_version: null },
  { kind: "mariadb", installed: ["11.4.13"], default_spec: null, default_version: null },
  { kind: "postgresql", installed: ["18.6"], default_spec: null, default_version: null },
];

const available: AvailableRelease[] = [
  { version: "8.5.12", lts: false },
  { version: "8.4.27", lts: false },
  { version: "8.3.36", lts: false },
  { version: "7.4.33", lts: false },
];

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  overview: () => overview(),
  service_action: ({ action, target }) => {
    if (action === "start") running = true;
    if (action === "shutdown") running = false;
    return overview(`${action}: ${target}`);
  },
  list_sites: () => sites,
  list_runtimes: () => runtimes,
  available_runtimes: () => available,
  install_runtime: () => new Promise((resolve) => setTimeout(() => resolve("8.4.27"), 1500)),
  create_site_database: ({ site }) => `Database \`${String(site).replace(/-/g, "_")}\` dibuat di postgresql@18 (port 5418).`,
  read_log: ({ service }) => `[contoh] log ${service}\n2026-10-26 12:56:15 0 [Note] Server ready for connections.`,
};

export function install() {
  const w = window as unknown as Record<string, unknown>;
  w.__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd.startsWith("plugin:event|")) return 0;
      const handler = handlers[cmd];
      if (!handler) throw `mock: command ${cmd} tidak ada`;
      return handler(args);
    },
    transformCallback: () => 0,
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
}
