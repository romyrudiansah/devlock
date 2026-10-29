// Bagian bersama halaman Beranda dan Service: data service, aksi, dan notifikasi.

import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type Overview, type ServiceAction, type ServiceStatus } from "../api";
import { t } from "../i18n";

export type ServiceGroup = ReturnType<typeof groupServices>[number];

/** Gabungkan worker `php@8.3#0..3` menjadi satu baris `php@8.3`. */
export function groupServices(services: ServiceStatus[]) {
  const groups = new Map<string, ServiceStatus[]>();
  for (const s of services) {
    groups.set(s.group, [...(groups.get(s.group) ?? []), s]);
  }
  return [...groups.entries()].map(([group, items]) => {
    const running = items.filter((s) => s.state === "running").length;
    const failed = items.find((s) => s.state === "failed");
    const starting = items.some((s) => s.state === "starting");
    const state =
      running === items.length
        ? "running"
        : failed
          ? "failed"
          : starting
            ? "starting"
            : running > 0
              ? "partial"
              : items[0].state;
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

/** Service dianggap menyala (toggle on) selama tidak berhenti/gagal. */
export const isOn = (g: ServiceGroup) => g.state === "running" || g.state === "partial" || g.state === "starting";

export function formatUptime(secs: number) {
  if (secs < 0) return "-";
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return t("{h}j {m}m", { h: Math.floor(secs / 3600), m: Math.floor((secs % 3600) / 60) });
}

export const STATE_LABEL: Record<string, string> = {
  running: "Berjalan",
  partial: "Sebagian",
  stopped: "Berhenti",
  restarting: "Restart…",
  starting: "Menyiapkan…",
  failed: "Gagal",
};

export const stateLabel = (state: string) => (STATE_LABEL[state] ? t(STATE_LABEL[state]) : state);

export type Notice = { ok: boolean; text: string };

/** Overview daemon yang diperbarui tiap 3 detik dan saat tray/watcher mengubah service. */
export function useOverview() {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);

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

  return { overview, busy, notice, setNotice, refresh, run };
}

/** Peringatan "CA belum dipercaya" tidak bisa ditutup: ada tombol untuk menyelesaikannya. */
const isCaWarning = (w: string) => w.includes("devlock ssl trust");

/** Peringatan yang ditutup diingat per teks (per perangkat), agar tidak muncul lagi tiap refresh. */
const DISMISSED_KEY = "dismissed-warnings";

function loadDismissed(): string[] {
  try {
    return JSON.parse(localStorage.getItem(DISMISSED_KEY) ?? "[]");
  } catch {
    return [];
  }
}

function saveDismissed(list: string[]) {
  try {
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(list));
  } catch {
    // localStorage tidak tersedia: hanya berlaku sampai jendela ditutup.
  }
}

function CloseButton({ onClick }: { onClick: () => void }) {
  return (
    <button className="close" title={t("Tutup notifikasi")} aria-label={t("Tutup notifikasi")} onClick={onClick}>
      ×
    </button>
  );
}

/** Hasil aksi terakhir dan peringatan daemon, sebagai InfoBar Fluent. */
export function Notices({
  overview,
  notice,
  setNotice,
  refresh,
}: Pick<ReturnType<typeof useOverview>, "overview" | "notice" | "setNotice" | "refresh">) {
  const [dismissed, setDismissed] = useState<string[]>(loadDismissed);
  const [trusting, setTrusting] = useState(false);

  const dismiss = (warning: string) => {
    const next = [...dismissed, warning];
    setDismissed(next);
    saveDismissed(next);
  };

  const trustCa = async () => {
    setTrusting(true);
    try {
      setNotice({ ok: true, text: await api.sslTrust() });
      await refresh();
    } catch (e) {
      setNotice({ ok: false, text: errorText(e) });
    } finally {
      setTrusting(false);
    }
  };

  // Peringatan yang sudah tidak dikirim daemon dilupakan, agar bisa muncul lagi bila terulang.
  useEffect(() => {
    if (!overview) return;
    const still = dismissed.filter((w) => overview.warnings.includes(w));
    if (still.length !== dismissed.length) {
      setDismissed(still);
      saveDismissed(still);
    }
  }, [overview, dismissed]);

  const warnings = (overview?.warnings ?? []).filter((w) => isCaWarning(w) || !dismissed.includes(w));

  return (
    <>
      {notice && (
        <div className={`notice with-actions ${notice.ok ? "ok" : "error"}`}>
          <span>{notice.text}</span>
          <CloseButton onClick={() => setNotice(null)} />
        </div>
      )}
      {warnings.map((w) => (
        <div key={w} className="notice warn with-actions">
          <span>{w}</span>
          {isCaWarning(w) ? (
            <button disabled={trusting} onClick={trustCa}>
              {trusting ? t("Menunggu konfirmasi Windows…") : t("Percayai CA DevLock")}
            </button>
          ) : (
            <CloseButton onClick={() => dismiss(w)} />
          )}
        </div>
      ))}
    </>
  );
}

/** Toggle Windows 11. */
export function Toggle({
  on,
  disabled,
  label,
  onChange,
}: {
  on: boolean;
  disabled?: boolean;
  label: string;
  onChange: (on: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      title={label}
      className={`toggle ${on ? "on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!on)}
    >
      <span className="knob" />
    </button>
  );
}
