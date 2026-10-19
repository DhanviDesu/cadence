import type { CloseOutDecision, TaskNode } from "./types";

/// Mirrors the backend's carry-over + ancestor-forcing rules client-side, so
/// the wizard can show an accurate "here's what tomorrow will look like"
/// preview without a round trip per keystroke. A not-yet-finished task
/// carries unless explicitly decided drop/done/backlog; an already
/// done/dropped task only carries if explicitly overridden with carry_over
/// (see default_decision/reopen_ids in close_out_day_into - same rule). A
/// parent with no carrying decision of its own still comes along
/// structurally if any of its children end up carrying.
export function computeTomorrowPreview(
  tasks: TaskNode[],
  decisions: Record<number, CloseOutDecision>,
): TaskNode[] {
  function visit(node: TaskNode): TaskNode | null {
    const children = node.children.map(visit).filter((n): n is TaskNode => n !== null);
    const alreadyFinished = node.status === "done" || node.status === "dropped";
    const action = decisions[node.id]?.action;
    const removedByDecision = action === "drop" || action === "done" || action === "backlog";
    const explicitlyCarried = action === "carry_over";
    const selfCarries = alreadyFinished ? explicitlyCarried : !removedByDecision;

    if (!selfCarries && children.length === 0) return null;
    // An explicit override reopens a finished task, same as the backend -
    // otherwise the preview would still show it looking done/dropped
    // despite the override appearing to have no effect.
    const status = alreadyFinished && explicitlyCarried ? "todo" : node.status;
    return { ...node, status, children };
  }
  return tasks.map(visit).filter((n): n is TaskNode => n !== null);
}
