// M1 scope only: the keys needed to type and organize today's list without a
// mouse. Close-out, backlog, search, and quick-add shortcuts land in later
// milestones once those features exist.
export type OutlinerAction =
  | { type: "new_sibling" }
  | { type: "indent" }
  | { type: "outdent" }
  | { type: "move_focus"; direction: "up" | "down" }
  | { type: "move_task"; direction: "up" | "down" }
  | { type: "set_status"; index: number }
  | { type: "toggle_done" }
  | { type: "delete" }
  | { type: "clear_override" };

export function resolveKeyAction(e: KeyboardEvent): OutlinerAction | null {
  const mod = e.metaKey || e.ctrlKey;

  if (e.key === "Enter" && !mod) return { type: "new_sibling" };
  if (e.key === "Enter" && mod) return { type: "toggle_done" };
  if (e.key === "Tab" && !e.shiftKey) return { type: "indent" };
  if (e.key === "Tab" && e.shiftKey) return { type: "outdent" };
  if (e.key === "ArrowUp" && e.altKey) return { type: "move_task", direction: "up" };
  if (e.key === "ArrowDown" && e.altKey) return { type: "move_task", direction: "down" };
  if (e.key === "ArrowUp" && !mod) return { type: "move_focus", direction: "up" };
  if (e.key === "ArrowDown" && !mod) return { type: "move_focus", direction: "down" };
  if (mod && e.key === "Backspace") return { type: "delete" };
  // Not ⌘0: macOS/WKWebView reserves that for zoom reset and eats the
  // keydown before it reaches the page, so it never fires here.
  if (mod && e.shiftKey && e.key.toLowerCase() === "o") return { type: "clear_override" };
  if (mod && /^[1-7]$/.test(e.key)) return { type: "set_status", index: Number(e.key) - 1 };

  return null;
}
