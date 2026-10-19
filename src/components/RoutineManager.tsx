import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { RoutineDef } from "../lib/types";

interface Props {
  onClose: () => void;
  onChange: () => void;
}

export function RoutineManager({ onClose, onChange }: Props) {
  const [routines, setRoutines] = useState<RoutineDef[] | null>(null);
  const [confirmingId, setConfirmingId] = useState<number | null>(null);
  const [editingItemId, setEditingItemId] = useState<number | null>(null);
  const itemInputRefs = useRef(new Map<number, HTMLInputElement>());
  const newRoutineInputRef = useRef<HTMLInputElement | null>(null);
  const editingInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    api.listRoutines().then(setRoutines);
  }, []);

  const addRoutine = async (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const before = new Set(routines?.map((r) => r.id) ?? []);
    const updated = await api.createRoutine(trimmed);
    setRoutines(updated);
    if (newRoutineInputRef.current) newRoutineInputRef.current.value = "";
    const created = updated.find((r) => !before.has(r.id));
    if (created) {
      requestAnimationFrame(() => itemInputRefs.current.get(created.id)?.focus());
    }
  };

  const addItem = async (routineId: number, text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    setRoutines(await api.addRoutineItem(routineId, trimmed));
    const input = itemInputRefs.current.get(routineId);
    if (input) input.value = "";
    onChange();
  };

  const deleteRoutine = async (routineId: number) => {
    setRoutines(await api.deleteRoutine(routineId));
    setConfirmingId(null);
    onChange();
  };

  const startEditItem = (itemId: number) => {
    setEditingItemId(itemId);
  };

  const saveEditItem = async () => {
    if (editingItemId == null) return;
    const id = editingItemId;
    const text = editingInputRef.current?.value.trim() ?? "";
    setEditingItemId(null);
    if (!text) return;
    setRoutines(await api.updateRoutineItem(id, text));
    onChange();
  };

  const deleteItem = async (itemId: number) => {
    setRoutines(await api.deleteRoutineItem(itemId));
    onChange();
  };

  const toggleAfterTasks = async (routineId: number, afterTasks: boolean) => {
    setRoutines(await api.setRoutineAfterTasks(routineId, afterTasks));
    onChange();
  };

  const moveRoutine = async (routineId: number, direction: "up" | "down") => {
    setRoutines(await api.moveRoutine(routineId, direction));
  };

  const moveItem = async (itemId: number, direction: "up" | "down") => {
    setRoutines(await api.moveRoutineItem(itemId, direction));
  };

  return (
    <div className="wizard-overlay">
      <div className="wizard routine-manager">
        <div className="wizard-progress">Manage Routines</div>
        {routines === null ? (
          <div>Loading…</div>
        ) : (
          <div className="routine-manager-scroll">
            {routines.map((r, routineIndex) => (
              <div key={r.id} className="routine-manager-block">
                <div className="routine-manager-header">
                  <div className="routine-manager-name">{r.name}</div>
                  <div className="reorder-buttons">
                    <button
                      className="reorder-button"
                      onClick={() => moveRoutine(r.id, "up")}
                      disabled={routineIndex === 0}
                      aria-label="Move routine up"
                    >
                      ↑
                    </button>
                    <button
                      className="reorder-button"
                      onClick={() => moveRoutine(r.id, "down")}
                      disabled={routineIndex === routines.length - 1}
                      aria-label="Move routine down"
                    >
                      ↓
                    </button>
                  </div>
                  <button
                    className="ghost-button"
                    onClick={() => toggleAfterTasks(r.id, !r.after_tasks)}
                    title="Where this routine shows up in the day"
                  >
                    {r.after_tasks ? "After tasks" : "Before tasks"}
                  </button>
                  {confirmingId === r.id ? (
                    <span className="routine-manager-confirm">
                      Delete this routine?
                      <button className="ghost-button" onClick={() => deleteRoutine(r.id)}>
                        Yes
                      </button>
                      <button className="ghost-button" onClick={() => setConfirmingId(null)}>
                        No
                      </button>
                    </span>
                  ) : (
                    <button className="ghost-button" onClick={() => setConfirmingId(r.id)}>
                      Delete
                    </button>
                  )}
                </div>
                <ul>
                  {r.items.map((item, itemIndex) => (
                    <li key={item.id} className="routine-item-row">
                      {editingItemId === item.id ? (
                        <input
                          ref={editingInputRef}
                          autoFocus
                          className="routine-item-edit"
                          // Uncontrolled: this input remounts fresh each time
                          // editing starts, so the item's current title is
                          // its one-time initial value - no state needed.
                          defaultValue={item.title}
                          onBlur={saveEditItem}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") saveEditItem();
                            else if (e.key === "Escape") setEditingItemId(null);
                          }}
                        />
                      ) : (
                        <span
                          className="routine-item-title"
                          onClick={() => startEditItem(item.id)}
                        >
                          {item.title}
                        </span>
                      )}
                      <div className="reorder-buttons">
                        <button
                          className="reorder-button"
                          onClick={() => moveItem(item.id, "up")}
                          disabled={itemIndex === 0}
                          aria-label="Move item up"
                        >
                          ↑
                        </button>
                        <button
                          className="reorder-button"
                          onClick={() => moveItem(item.id, "down")}
                          disabled={itemIndex === r.items.length - 1}
                          aria-label="Move item down"
                        >
                          ↓
                        </button>
                      </div>
                      <button
                        className="routine-item-delete"
                        onClick={() => deleteItem(item.id)}
                        aria-label="Delete item"
                      >
                        ×
                      </button>
                    </li>
                  ))}
                </ul>
                <input
                  ref={(el) => {
                    if (el) itemInputRefs.current.set(r.id, el);
                    else itemInputRefs.current.delete(r.id);
                  }}
                  placeholder="Add item + Enter"
                  defaultValue=""
                  onKeyDown={(e) => {
                    if (e.key === "Enter") addItem(r.id, e.currentTarget.value);
                  }}
                />
              </div>
            ))}
            <div className="routine-manager-new">
              <input
                ref={newRoutineInputRef}
                placeholder="New routine name (e.g. Morning)"
                defaultValue=""
                onKeyDown={(e) => {
                  if (e.key === "Enter") addRoutine(e.currentTarget.value);
                }}
              />
            </div>
          </div>
        )}
        <button className="wizard-cancel" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
