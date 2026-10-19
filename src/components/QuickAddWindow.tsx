import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import { STATUS_LABEL, STATUS_ORDER } from "../lib/types";
import type { Status } from "../lib/types";

// Mirrors what the backend's inline parser already accepts in parens - "not
// started" has no paren form since that's just the default, untouched state.
const STATUS_PAREN: Partial<Record<Status, string>> = {
  wip: "wip",
  in_review: "in review",
  submitting: "submitting",
  done: "done",
  blocked: "blocked",
  dropped: "dropped",
};

export function QuickAddWindow() {
  const [title, setTitle] = useState("");
  const [status, setStatus] = useState<Status | null>(null);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // The window is shown/hidden rather than recreated, so re-focus the input
  // every time it becomes visible again instead of only on first mount.
  useEffect(() => {
    const win = getCurrentWindow();
    const focusAndSelect = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
    };
    focusAndSelect();
    // Dismiss on click-away, same as Spotlight-style capture windows do.
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) focusAndSelect();
      else win.hide();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const hide = () => {
    setTitle("");
    setStatus(null);
    // Input is uncontrolled (see below), so clearing `title` state alone
    // wouldn't clear the visible text - reach into the DOM directly too.
    if (inputRef.current) inputRef.current.value = "";
    // Deferred a tick: hiding synchronously from inside the same keydown
    // that triggered it was unreliable for Escape specifically.
    setTimeout(() => getCurrentWindow().hide(), 0);
  };

  const submit = async () => {
    const text = title.trim();
    if (!text || saving) return;
    setSaving(true);
    try {
      const paren = status ? STATUS_PAREN[status] : undefined;
      const finalTitle = paren ? `${text} (${paren})` : text;
      await api.createTask(null, null, finalTitle);
      await emit("task-added");
    } finally {
      setSaving(false);
      hide();
    }
  };

  // Window-level capture listener, same pattern as every other overlay in
  // the app - the input's own onKeyDown alone did not reliably catch Escape.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Enter") {
        e.preventDefault();
        submit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        hide();
      } else if ((e.metaKey || e.ctrlKey) && /^[1-7]$/.test(e.key)) {
        e.preventDefault();
        setStatus(STATUS_ORDER[Number(e.key) - 1]);
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [title, status, saving]);

  return (
    <div className="quickadd">
      <input
        ref={inputRef}
        className="quickadd-input"
        placeholder="New task… (⌘1-7 status, Enter to add, Esc to dismiss)"
        // Uncontrolled, same reasoning as TaskRow: `onChange` still tracks
        // `title` for submit()/keyboard handlers, it just never feeds back
        // into `value`, so native per-field undo stays intact while typing.
        defaultValue=""
        disabled={saving}
        onChange={(e) => setTitle(e.target.value)}
      />
      {status && <span className="quickadd-status">{STATUS_LABEL[status]}</span>}
    </div>
  );
}
