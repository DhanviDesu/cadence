import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { BacklogItem, DayPayload } from "../lib/types";

interface Props {
  onClose: () => void;
  onPulled: (day: DayPayload) => void;
}

export function BacklogView({ onClose, onPulled }: Props) {
  const [items, setItems] = useState<BacklogItem[] | null>(null);
  const [confirmingId, setConfirmingId] = useState<number | null>(null);

  useEffect(() => {
    api.getBacklog().then(setItems);
  }, []);

  const pull = async (id: number) => {
    const day = await api.pullFromBacklog(id);
    setItems((prev) => prev?.filter((i) => i.id !== id) ?? null);
    onPulled(day);
  };

  const changeSnooze = async (id: number, date: string) => {
    setItems(await api.setBacklogSnooze(id, date || null));
  };

  const drop = async (id: number) => {
    setItems(await api.dropFromBacklog(id));
    setConfirmingId(null);
  };

  return (
    <div className="wizard-overlay">
      <div className="wizard wizard-wide backlog-view">
        <div className="wizard-progress">Backlog</div>
        {items === null ? (
          <div className="preview-empty">Loading…</div>
        ) : items.length === 0 ? (
          <div className="preview-empty">Nothing parked right now.</div>
        ) : (
          <div className="backlog-scroll">
            {items.map((item) => (
              <div key={item.id} className="backlog-row">
                <div className="backlog-row-main">
                  {item.breadcrumb.length > 0 && (
                    <div className="wizard-breadcrumb">under: {[...item.breadcrumb].reverse().join(" > ")}</div>
                  )}
                  <div className="backlog-title">{item.title || "(untitled)"}</div>
                </div>
                <div className="backlog-row-actions">
                  <label className="backlog-snooze-label">
                    Surface on
                    <input
                      type="date"
                      value={item.snooze_until ?? ""}
                      onChange={(e) => changeSnooze(item.id, e.target.value)}
                    />
                  </label>
                  <button className="ghost-button" onClick={() => pull(item.id)}>
                    Pull to Today
                  </button>
                  {confirmingId === item.id ? (
                    <span className="routine-manager-confirm">
                      Drop it?
                      <button className="ghost-button" onClick={() => drop(item.id)}>
                        Yes
                      </button>
                      <button className="ghost-button" onClick={() => setConfirmingId(null)}>
                        No
                      </button>
                    </span>
                  ) : (
                    <button className="ghost-button" onClick={() => setConfirmingId(item.id)}>
                      Drop
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
        <button className="wizard-cancel" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
