import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { resolveKeyAction } from "../lib/keymap";
import { STATUS_ORDER } from "../lib/types";
import type { DayPayload, Status, TaskNode } from "../lib/types";

function flatten(tasks: TaskNode[]): TaskNode[] {
  const out: TaskNode[] = [];
  const walk = (nodes: TaskNode[]) => {
    for (const n of nodes) {
      out.push(n);
      walk(n.children);
    }
  };
  walk(tasks);
  return out;
}

// One entry per undoable structural action, pushed right before the API
// call that performs it. ⌘Z pops the most recent entry and replays its
// inverse. Each case is a real inverse of its forward operation (indent's
// inverse is outdent on the same task, "up" undoes "down", etc.) except
// "delete", which can't be reversed server-side - that one recreates the
// subtree from scratch via normal commands, so it gets a new task id. That
// means an older stack entry referencing a deleted-then-restored task's
// original id can go stale; this is a known, accepted limitation rather
// than something worth a full id-remapping system for.
type UndoEntry =
  | { type: "create"; taskId: number }
  | { type: "indent"; taskId: number }
  | { type: "outdent"; taskId: number }
  | { type: "move"; taskId: number; direction: "up" | "down" }
  | { type: "status"; taskId: number; prevStatus: Status }
  | { type: "delete"; task: TaskNode; parentId: number | null };

const UNDO_STACK_LIMIT = 50;

export function useOutliner() {
  const [day, setDay] = useState<DayPayload | null>(null);
  const [undoStack, setUndoStack] = useState<UndoEntry[]>([]);
  // Unsaved per-task edits, keyed by task id. A task with no entry here
  // displays its server title. Kept centrally (rather than as local state
  // inside each row) so a commit can clear the draft the instant it's sent,
  // with no dependency on focus/blur timing - see commitDraft below.
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const inputRefs = useRef(new Map<number, HTMLInputElement>());
  const pendingFocusId = useRef<number | null | undefined>(undefined);

  useEffect(() => {
    api.getToday().then(setDay);
  }, []);

  useEffect(() => {
    if (pendingFocusId.current === undefined) return;
    const id = pendingFocusId.current;
    pendingFocusId.current = undefined;
    if (id != null) {
      requestAnimationFrame(() => {
        const el = inputRefs.current.get(id);
        el?.focus();
        el?.setSelectionRange(el.value.length, el.value.length);
      });
    }
  }, [day]);

  const registerInput = useCallback((id: number, el: HTMLInputElement | null) => {
    if (el) inputRefs.current.set(id, el);
    else inputRefs.current.delete(id);
  }, []);

  const pushUndo = useCallback((entry: UndoEntry) => {
    setUndoStack((s) => [...s.slice(-(UNDO_STACK_LIMIT - 1)), entry]);
  }, []);

  const applyUpdate = useCallback((updated: DayPayload, focusId?: number | null) => {
    if (focusId !== undefined) pendingFocusId.current = focusId;
    setDay(updated);
  }, []);

  const setDraft = useCallback((id: number, value: string) => {
    setDrafts((d) => ({ ...d, [id]: value }));
  }, []);

  const clearDraft = useCallback((id: number) => {
    setDrafts((d) => {
      if (!(id in d)) return d;
      const next = { ...d };
      delete next[id];
      return next;
    });
  }, []);

  // Commits a task's draft if present, clearing it immediately (before the
  // network round-trip resolves) so a blur that fires late - e.g. after focus
  // has already moved on to a newly-created row - can never re-send stale
  // text over a title the server already parsed and cleaned up. Returns the
  // authoritative title afterward, so the caller can sync its (uncontrolled)
  // input's DOM value directly - see TaskRow, which never feeds this back
  // through React's `value` prop during normal typing, since that's what
  // silently wipes the browser's native per-field undo history on every
  // keystroke.
  const commitDraft = useCallback(
    async (task: TaskNode): Promise<string> => {
      const draft = drafts[task.id];
      if (draft === undefined) return task.title;
      clearDraft(task.id);
      if (draft === task.title) return draft;
      const updated = await api.updateTaskTitle(task.id, draft);
      setDay(updated);
      return flatten(updated.tasks).find((t) => t.id === task.id)?.title ?? draft;
    },
    [drafts, clearDraft],
  );

  const addFirstTask = useCallback(async () => {
    const updated = await api.createTask(null, null, "");
    const created = flatten(updated.tasks).slice(-1)[0];
    if (created) pushUndo({ type: "create", taskId: created.id });
    applyUpdate(updated, created?.id ?? null);
  }, [applyUpdate, pushUndo]);

  const handleKeyDown = useCallback(
    async (e: React.KeyboardEvent<HTMLInputElement>, task: TaskNode) => {
      const liveTitle = drafts[task.id] ?? task.title;
      let action = resolveKeyAction(e.nativeEvent);
      const bareBackspace = e.key === "Backspace" && !e.metaKey && !e.ctrlKey;
      if (!action && bareBackspace && liveTitle === "") {
        action = { type: "delete" };
      }
      if (!action) return;
      e.preventDefault();

      if (action.type !== "delete") {
        await commitDraft(task);
      }

      const flat = day ? flatten(day.tasks) : [];

      switch (action.type) {
        case "new_sibling": {
          const before = day ? flatten(day.tasks).map((t) => t.id) : [];
          const updated = await api.createTask(task.parent_id, task.id, "");
          const created = flatten(updated.tasks).find((t) => !before.includes(t.id));
          if (created) pushUndo({ type: "create", taskId: created.id });
          applyUpdate(updated, created?.id ?? null);
          break;
        }
        case "indent": {
          pushUndo({ type: "indent", taskId: task.id });
          const updated = await api.indentTask(task.id);
          applyUpdate(updated, task.id);
          break;
        }
        case "outdent": {
          pushUndo({ type: "outdent", taskId: task.id });
          const updated = await api.outdentTask(task.id);
          applyUpdate(updated, task.id);
          break;
        }
        case "move_task": {
          pushUndo({ type: "move", taskId: task.id, direction: action.direction });
          const updated = await api.moveTask(task.id, action.direction);
          applyUpdate(updated, task.id);
          break;
        }
        case "move_focus": {
          const idx = flat.findIndex((t) => t.id === task.id);
          const next = flat[idx + (action.direction === "up" ? -1 : 1)];
          if (next) inputRefs.current.get(next.id)?.focus();
          break;
        }
        case "set_status": {
          const status = STATUS_ORDER[action.index];
          if (status) {
            pushUndo({ type: "status", taskId: task.id, prevStatus: task.status });
            const updated = await api.setStatus(task.id, status);
            applyUpdate(updated, task.id);
          }
          break;
        }
        case "toggle_done": {
          pushUndo({ type: "status", taskId: task.id, prevStatus: task.status });
          const updated = await api.toggleDone(task.id);
          applyUpdate(updated, task.id);
          break;
        }
        case "clear_override": {
          pushUndo({ type: "status", taskId: task.id, prevStatus: task.status });
          const updated = await api.clearStatusOverride(task.id);
          applyUpdate(updated, task.id);
          break;
        }
        case "delete": {
          const idx = flat.findIndex((t) => t.id === task.id);
          const fallback = flat[idx - 1] ?? flat[idx + 1] ?? null;
          clearDraft(task.id);
          pushUndo({ type: "delete", task, parentId: task.parent_id });
          const updated = await api.deleteTask(task.id);
          applyUpdate(updated, fallback?.id ?? null);
          break;
        }
      }
    },
    [day, drafts, applyUpdate, commitDraft, clearDraft, pushUndo],
  );

  const replaceDay = useCallback((updated: DayPayload) => {
    setDrafts({});
    // Undo entries reference task ids within the currently open day; a
    // close-out, import, or cross-window refresh can swap that day out
    // from under them, so the stack no longer means anything afterward.
    setUndoStack([]);
    setDay(updated);
  }, []);

  // Pops the most recent undoable action and replays its inverse. Every
  // case but "delete" is a real inverse call to the backend; "delete"
  // instead recreates the subtree from scratch via normal commands (see the
  // UndoEntry comment above for why that one gets a new task id).
  const undo = useCallback(async () => {
    if (undoStack.length === 0) return;
    const entry = undoStack[undoStack.length - 1];
    setUndoStack((s) => s.slice(0, -1));

    try {
      switch (entry.type) {
        case "create": {
          const updated = await api.deleteTask(entry.taskId);
          applyUpdate(updated, null);
          break;
        }
        case "indent": {
          const updated = await api.outdentTask(entry.taskId);
          applyUpdate(updated, entry.taskId);
          break;
        }
        case "outdent": {
          const updated = await api.indentTask(entry.taskId);
          applyUpdate(updated, entry.taskId);
          break;
        }
        case "move": {
          const updated = await api.moveTask(entry.taskId, entry.direction === "up" ? "down" : "up");
          applyUpdate(updated, entry.taskId);
          break;
        }
        case "status": {
          // Always restore the raw status value directly - the backend
          // recomputes status_override as a side effect of setStatus based
          // on whether the task currently has children, so there's no
          // separate override flag worth tracking here. (It's only ever 1
          // for parent tasks in the first place - see set_status.)
          const updated = await api.setStatus(entry.taskId, entry.prevStatus);
          applyUpdate(updated, entry.taskId);
          break;
        }
        case "delete": {
          const seen = new Set(day ? flatten(day.tasks).map((t) => t.id) : []);
          let latestDay = day;

          async function restore(node: TaskNode, pid: number | null): Promise<number> {
            const updated = await api.createTask(pid, null, node.title);
            latestDay = updated;
            const createdId = flatten(updated.tasks).find((t) => !seen.has(t.id))!.id;
            seen.add(createdId);
            if (node.status !== "todo") {
              latestDay = await api.setStatus(createdId, node.status);
            }
            for (const child of node.children) {
              await restore(child, createdId);
            }
            return createdId;
          }

          const restoredId = await restore(entry.task, entry.parentId);
          if (latestDay) applyUpdate(latestDay, restoredId);
          break;
        }
      }
    } catch (err) {
      // Most likely a stale id from an older entry pointing at a task that
      // no longer exists (e.g. a delete further down the stack already got
      // undone with a new id) - see the UndoEntry comment. Not fatal.
      console.warn("undo failed, skipping", entry, err);
    }
  }, [undoStack, day, applyUpdate]);

  // Only surface the "Deleted X, Undo" banner when ⌘Z would actually undo
  // that delete right now - i.e. it's still the top of the stack.
  const lastDeleted = undoStack.length > 0 && undoStack[undoStack.length - 1].type === "delete"
    ? (undoStack[undoStack.length - 1] as { type: "delete"; task: TaskNode; parentId: number | null })
    : null;

  return {
    day,
    drafts,
    registerInput,
    handleKeyDown,
    setDraft,
    commitDraft,
    addFirstTask,
    replaceDay,
    lastDeleted,
    undo,
  };
}
