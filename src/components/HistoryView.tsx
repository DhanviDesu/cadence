import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { addDays, formatDateLong } from "../lib/date";
import type { DayPayload } from "../lib/types";
import { PreviewTree } from "./PreviewTree";

interface Props {
  liveDate: string;
  initialDate?: string;
  onClose: () => void;
}

export function HistoryView({ liveDate, initialDate, onClose }: Props) {
  const [viewDate, setViewDate] = useState(() => initialDate ?? addDays(liveDate, -1));
  const [day, setDay] = useState<DayPayload | null>(null);

  useEffect(() => {
    let cancelled = false;
    setDay(null);
    api.getDayByDate(viewDate).then((d) => {
      if (!cancelled) setDay(d);
    });
    return () => {
      cancelled = true;
    };
  }, [viewDate]);

  const canGoForward = viewDate < liveDate;

  const goBack = () => setViewDate((d) => addDays(d, -1));
  const goForward = () => setViewDate((d) => (d < liveDate ? addDays(d, 1) : d));

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === "[") {
        e.preventDefault();
        goBack();
      } else if (mod && e.key === "]") {
        e.preventDefault();
        goForward();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewDate, liveDate]);

  return (
    <div className="wizard-overlay">
      <div className="wizard wizard-wide history-view">
        <div className="wizard-progress">History (read-only)</div>
        <div className="history-nav">
          <button className="ghost-button" onClick={goBack}>
            [⌘[] ← Previous
          </button>
          <div className="history-date">{formatDateLong(viewDate)}</div>
          <button className="ghost-button" onClick={goForward} disabled={!canGoForward}>
            Next → [⌘]]
          </button>
        </div>
        <div className="history-jump">
          <input
            type="date"
            value={viewDate}
            max={liveDate}
            onChange={(e) => {
              if (e.target.value) setViewDate(e.target.value > liveDate ? liveDate : e.target.value);
            }}
          />
        </div>

        <div className="history-body">
          {day === null ? (
            <div className="preview-empty">Loading…</div>
          ) : (
            <PreviewTree tasks={day.tasks} />
          )}
        </div>

        <button className="wizard-cancel" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
