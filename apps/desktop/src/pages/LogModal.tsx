import { useEffect, useRef, useState } from "react";
import { api, errorText } from "../api";

export default function LogModal({ service, onClose }: { service: string; onClose: () => void }) {
  const [text, setText] = useState("Memuat…");
  const [follow, setFollow] = useState(true);
  const pre = useRef<HTMLPreElement>(null);

  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const log = await api.readLog(service);
        if (alive) setText(log || "(log masih kosong)");
      } catch (e) {
        if (alive) setText(errorText(e));
      }
    };
    load();
    const timer = follow ? setInterval(load, 2000) : undefined;
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [service, follow]);

  useEffect(() => {
    if (follow && pre.current) pre.current.scrollTop = pre.current.scrollHeight;
  }, [text, follow]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <h2>Log {service}</h2>
          <label className="small">
            <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />{" "}
            Ikuti
          </label>
          <button className="ghost" onClick={onClose}>
            Tutup
          </button>
        </header>
        <pre ref={pre} className="log">
          {text}
        </pre>
      </div>
    </div>
  );
}
