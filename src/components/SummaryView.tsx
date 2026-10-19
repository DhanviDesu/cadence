import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { addDays } from "../lib/date";
import type { RangeSummary, RangeSummaryItem } from "../lib/types";
import { StatusChip } from "./StatusChip";

interface Props {
  liveDate: string;
  onClose: () => void;
}

const PRESETS: { label: string; days: number }[] = [
  { label: "Last 7 days", days: 7 },
  { label: "Last 14 days", days: 14 },
  { label: "Last 30 days", days: 30 },
];

function ItemRow({ item }: { item: RangeSummaryItem }) {
  return (
    <div className="summary-row">
      {item.breadcrumb.length > 0 && (
        <div className="wizard-breadcrumb">under: {[...item.breadcrumb].reverse().join(" > ")}</div>
      )}
      <div className="summary-row-title">
        {item.title || "(untitled)"}
        <StatusChip status={item.status} derived={false} />
      </div>
    </div>
  );
}

function Section({ title, items }: { title: string; items: RangeSummaryItem[] }) {
  return (
    <div className="summary-section">
      <div className="summary-section-header">
        {title} <span className="summary-count">{items.length}</span>
      </div>
      {items.length === 0 ? (
        <div className="preview-empty">None.</div>
      ) : (
        items.map((item) => <ItemRow key={item.id} item={item} />)
      )}
    </div>
  );
}

export function SummaryView({ liveDate, onClose }: Props) {
  const [start, setStart] = useState(() => addDays(liveDate, -6));
  const [end, setEnd] = useState(liveDate);
  const [summary, setSummary] = useState<RangeSummary | null>(null);

  useEffect(() => {
    if (start > end) return;
    api.summarizeRange(start, end).then(setSummary);
  }, [start, end]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div className="wizard-overlay">
      <div className="wizard wizard-wide summary-view">
        <div className="wizard-progress">Summary</div>

        <div className="summary-presets">
          {PRESETS.map((p) => (
            <button key={p.label} className="ghost-button" onClick={() => { setStart(addDays(liveDate, -(p.days - 1))); setEnd(liveDate); }}>
              {p.label}
            </button>
          ))}
        </div>

        <div className="summary-range">
          <input type="date" value={start} max={end} onChange={(e) => e.target.value && setStart(e.target.value)} />
          <span>to</span>
          <input type="date" value={end} max={liveDate} onChange={(e) => e.target.value && setEnd(e.target.value)} />
        </div>

        {start > end ? (
          <div className="wizard-stale">Start date must be before the end date.</div>
        ) : summary === null ? (
          <div className="preview-empty">Loading…</div>
        ) : (
          <div className="summary-scroll">
            <Section title="Completed" items={summary.completed} />
            <Section title="Dropped" items={summary.dropped} />
            <Section title="Still in progress" items={summary.still_active} />
            <Section title="Created" items={summary.created} />
          </div>
        )}

        <button className="wizard-cancel" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
