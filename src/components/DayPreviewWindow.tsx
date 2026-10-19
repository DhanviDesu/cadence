import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../lib/api";
import { formatDateLong } from "../lib/date";
import type { DayPayload } from "../lib/types";
import { PreviewTree } from "./PreviewTree";

export function DayPreviewWindow() {
  const [day, setDay] = useState<DayPayload | null>(null);

  useEffect(() => {
    const win = getCurrentWindow();
    const refresh = () => {
      api.getToday().then(setDay);
    };
    refresh();
    // A fresh webview with no persistent state - re-fetch every time it's
    // shown again rather than only once, since today's list may have
    // changed since the last glance.
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) refresh();
      else win.hide();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        setTimeout(() => getCurrentWindow().hide(), 0);
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  return (
    <div className="preview-popup">
      <div className="preview-popup-header">{day ? formatDateLong(day.date) : "Today"}</div>
      <div className="preview-popup-body">
        {day === null ? (
          <div className="preview-empty">Loading…</div>
        ) : day.tasks.length === 0 ? (
          <div className="preview-empty">Nothing on today's list.</div>
        ) : (
          <PreviewTree tasks={day.tasks} />
        )}
      </div>
    </div>
  );
}
