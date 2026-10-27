import { invoke } from "@tauri-apps/api/core";

export interface ServiceStatus {
  name: string;
  group: string;
  state: "running" | "starting" | "stopped" | "restarting" | "failed";
  pid: number | null;
  port: number | null;
  uptime_secs: number | null;
  restarts: number;
  error: string | null;
  log: string;
}

export interface Overview {
  root: string;
  www: string;
  tld: string;
  http_port: number;
  daemon_running: boolean;
  services: ServiceStatus[];
  warnings: string[];
  message: string | null;
  ok: boolean;
}

export interface SiteInfo {
  name: string;
  url: string;
  domains: string[];
  kind: "php" | "proxy" | "static";
  detail: string;
  database: string | null;
  dir: string;
  docroot: string;
  warning: string | null;
}

export interface RuntimeInfo {
  kind: string;
  installed: string[];
  default_spec: string | null;
  default_version: string | null;
}

export interface AvailableRelease {
  version: string;
  lts: boolean;
}

export interface InstallProgress {
  kind: string;
  version: string;
  stage: string;
  downloaded: number;
  total: number | null;
}

export type ServiceAction = "start" | "stop" | "restart" | "reload" | "shutdown";

export const api = {
  overview: () => invoke<Overview>("overview"),
  serviceAction: (action: ServiceAction, target = "all", updateHosts = true) =>
    invoke<Overview>("service_action", { action, target, updateHosts }),
  listSites: () => invoke<SiteInfo[]>("list_sites"),
  listRuntimes: () => invoke<RuntimeInfo[]>("list_runtimes"),
  availableRuntimes: (kind: string) => invoke<AvailableRelease[]>("available_runtimes", { kind }),
  installRuntime: (kind: string, version: string) =>
    invoke<string>("install_runtime", { kind, version }),
  uninstallRuntime: (kind: string, version: string) =>
    invoke<void>("uninstall_runtime", { kind, version }),
  setDefaultRuntime: (kind: string, version: string) =>
    invoke<void>("set_default_runtime", { kind, version }),
  readLog: (service: string, lines = 300) => invoke<string>("read_log", { service, lines }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  openTerminal: (dir: string) => invoke<void>("open_terminal", { dir }),
  createSiteDatabase: (site: string) => invoke<string>("create_site_database", { site }),
};

/** Pesan error dari command Tauri (string) atau Error biasa. */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
