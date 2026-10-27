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

export interface PhpExtension {
  name: string;
  enabled: boolean;
  zend: boolean;
}

export interface PhpInfo {
  version: string;
  ini_path: string;
  extensions: PhpExtension[];
  xdebug_installed: boolean;
  xdebug_enabled: boolean;
  ini: [string, string | null][];
}

export interface Settings {
  www: string;
  tld: string;
  http_port: number;
  https_enabled: boolean;
  https_port: number;
  php_workers: number;
  autostart_services: boolean;
  start_with_windows: boolean;
}

export interface TemplateInfo {
  id: string;
  label: string;
  uses_php: boolean;
  uses_node: boolean;
  needs_database: boolean;
}

export interface CreatedProject {
  dir: string;
  url: string;
  database: string | null;
  notes: string[];
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
  phpInfo: (version: string) => invoke<PhpInfo>("php_info", { version }),
  phpSetExtension: (version: string, name: string, enabled: boolean) =>
    invoke<PhpInfo>("php_set_extension", { version, name, enabled }),
  phpXdebug: (version: string, action: "install" | "on" | "off") =>
    invoke<PhpInfo>("php_xdebug", { version, action }),
  phpSetIni: (version: string, values: [string, string][]) =>
    invoke<PhpInfo>("php_set_ini", { version, values }),
  openPhpIni: (version: string) => invoke<void>("open_php_ini", { version }),
  listTemplates: () => invoke<TemplateInfo[]>("list_templates"),
  createProject: (p: { template: string; name: string; php?: string; node?: string; database?: string }) =>
    invoke<CreatedProject>("create_project", p),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<string>("save_settings", { settings }),
};

/** Pesan error dari command Tauri (string) atau Error biasa. */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
