import type { CloseOutAction, CloseOutDecision, TaskNode } from "../lib/types";

interface Props {
  tasks: TaskNode[];
  depth?: number;
  highlightId?: number;
  decisions?: Record<number, CloseOutDecision>;
}

const ACTION_LABEL: Record<CloseOutAction, string> = {
  carry_over: "Carried",
  drop: "Dropped",
  done: "Done",
  backlog: "Backlog",
};

export function PreviewTree({ tasks, depth = 0, highlightId, decisions }: Props) {
  if (tasks.length === 0 && depth === 0) {
    return <div className="preview-empty">Nothing here.</div>;
  }
  return (
    <>
      {tasks.map((t) => {
        const decision = decisions?.[t.id]?.action;
        const finished = t.status === "done" || t.status === "dropped";
        return (
          <div key={t.id}>
            <div
              className={`preview-row${t.id === highlightId ? " preview-row-current" : ""}${finished ? " preview-row-finished" : ""}`}
              style={{ paddingLeft: `${depth * 16}px` }}
            >
              <span className={`bullet status-dot-${t.status}`} />
              <span className="preview-title">{t.title || "(untitled)"}</span>
              {decision && <span className={`preview-decision preview-decision-${decision}`}>{ACTION_LABEL[decision]}</span>}
            </div>
            {t.children.length > 0 && (
              <PreviewTree tasks={t.children} depth={depth + 1} highlightId={highlightId} decisions={decisions} />
            )}
          </div>
        );
      })}
    </>
  );
}
