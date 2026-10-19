import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { computeTomorrowPreview } from "../lib/closeOutPreview";
import type { CloseOutAction, CloseOutCandidate, CloseOutDecision, CloseOutResult, TaskNode } from "../lib/types";
import { PreviewTree } from "./PreviewTree";

interface Props {
  today: TaskNode[];
  todayDate: string;
  onDone: (result: CloseOutResult) => void;
  onCancel: () => void;
}

function nextDateLabel(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  const next = new Date(y, m - 1, d + 1);
  return next.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
}

const SUMMARY_LABEL: Record<CloseOutAction, string> = {
  carry_over: "carried",
  drop: "dropped",
  done: "done",
  backlog: "backlogged",
};

export function CloseOutWizard({ today, todayDate, onDone, onCancel }: Props) {
  const [candidates, setCandidates] = useState<CloseOutCandidate[] | null>(null);
  const [index, setIndex] = useState(0);
  // How far the linear "decide each one" flow has actually reached (0..=
  // candidates.length). Arrow keys let you browse back into anything below
  // this without re-deciding it - only C/D/B/Enter change a decision.
  const [furthest, setFurthest] = useState(0);
  const [decisions, setDecisions] = useState<Record<number, CloseOutDecision>>({});
  const [running, setRunning] = useState(false);
  const [showBacklogDate, setShowBacklogDate] = useState(false);
  const [snoozeDate, setSnoozeDate] = useState("");
  const dateInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    // A task row's input may still be focused from before the wizard opened -
    // blur it so keyboard shortcuts below don't also type into it.
    (document.activeElement as HTMLElement | null)?.blur();
    api.getCloseOutCandidates().then(setCandidates);
  }, []);

  useEffect(() => {
    if (showBacklogDate) dateInputRef.current?.focus();
  }, [showBacklogDate]);

  const tomorrowPreview = useMemo(() => computeTomorrowPreview(today, decisions), [today, decisions]);

  // Plain refs, not state: checking/setting them is immediate and shared
  // across every closure regardless of which render created it, so a
  // duplicate call arriving before React re-renders (e.g. a keydown's
  // default button-activation slipping through alongside our own handler)
  // can't double-submit or push `index` past the end of the array.
  const busyRef = useRef(false);

  const finish = async () => {
    setRunning(true);
    const result = await api.runCloseOut(decisions);
    onDone(result);
  };

  // Once every candidate has a decision (or there were none to begin with),
  // stop and let the user look at the preview before anything is committed.
  const inReview = candidates !== null && index >= candidates.length;

  const decide = (decision: CloseOutDecision) => {
    if (!candidates || running || busyRef.current) return;
    busyRef.current = true;
    (document.activeElement as HTMLElement | null)?.blur();
    const current = candidates[index];
    setDecisions((d) => ({ ...d, [current.id]: decision }));
    setShowBacklogDate(false);
    setSnoozeDate("");
    // Only the frontier item advances the flow; revising something you
    // arrow'd back to just updates it in place so you can see the change.
    if (index === furthest) {
      setFurthest(index + 1);
      setIndex(index + 1);
    }
    busyRef.current = false;
  };

  const back = () => {
    if (running || busyRef.current) return;
    setShowBacklogDate(false);
    setIndex((i) => Math.max(0, i - 1));
  };

  const forward = () => {
    if (running || busyRef.current) return;
    setShowBacklogDate(false);
    setIndex((i) => Math.min(furthest, i + 1));
  };

  const confirmBacklog = () => decide({ action: "backlog", snoozeUntil: snoozeDate || undefined });

  // Captures shortcuts before they can reach a background input; when the
  // backlog date field is open its own handlers take over instead.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (running || showBacklogDate) return;
      const key = e.key.toLowerCase();
      const relevant =
        key === "c" ||
        key === "d" ||
        key === "b" ||
        e.key === "Enter" ||
        e.key === "Backspace" ||
        e.key === "Escape" ||
        e.key === "ArrowUp" ||
        e.key === "ArrowDown";
      if (relevant) {
        e.preventDefault();
        e.stopPropagation();
      }
      if (e.key === "ArrowUp") return back();
      if (e.key === "ArrowDown") return forward();
      if (inReview) {
        if (e.key === "Enter") finish();
        else if (e.key === "Backspace") back();
        else if (e.key === "Escape") onCancel();
        return;
      }
      if (key === "c") decide({ action: "carry_over" });
      else if (key === "d") decide({ action: "drop" });
      else if (key === "b") setShowBacklogDate(true);
      else if (e.key === "Enter") decide({ action: "done" });
      else if (e.key === "Backspace") back();
      else if (e.key === "Escape") onCancel();
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  if (candidates === null || running) {
    return (
      <div className="wizard-overlay">
        <div className="wizard">Closing out…</div>
      </div>
    );
  }

  const columns = (highlightId?: number) => (
    <div className="wizard-columns">
      <div className="wizard-column">
        <div className="wizard-column-header">Today</div>
        <div className="wizard-column-body">
          <PreviewTree tasks={today} highlightId={highlightId} decisions={decisions} />
        </div>
      </div>
      <div className="wizard-column">
        <div className="wizard-column-header">{nextDateLabel(todayDate)} (preview)</div>
        <div className="wizard-column-body">
          <PreviewTree tasks={tomorrowPreview} />
        </div>
      </div>
    </div>
  );

  if (inReview) {
    const counts: Record<CloseOutAction, number> = { carry_over: 0, drop: 0, done: 0, backlog: 0 };
    for (const c of candidates) {
      // Mirrors the backend's default_decision: an untouched task defaults
      // to its own current status (done/dropped stay put), not carry_over -
      // otherwise this summary would claim an untouched done/dropped task
      // is carrying forward when it actually isn't.
      const defaultAction = c.status === "dropped" ? "drop" : c.status === "done" ? "done" : "carry_over";
      counts[decisions[c.id]?.action ?? defaultAction]++;
    }
    const summary = (Object.keys(counts) as CloseOutAction[])
      .filter((a) => counts[a] > 0)
      .map((a) => `${counts[a]} ${SUMMARY_LABEL[a]}`)
      .join(" · ");

    return (
      <div className="wizard-overlay">
        <div className="wizard wizard-wide">
          <div className="wizard-progress">Ready to close out</div>
          <div className="wizard-title">{summary || "Nothing to decide - everything carries over as is."}</div>
          {columns(undefined)}
          <div className="wizard-actions">
            <button onClick={finish}>[↵] Confirm & Close Out</button>
            <button onClick={back} disabled={candidates.length === 0}>
              [⌫] Back
            </button>
          </div>
          <button className="wizard-cancel" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    );
  }

  const current = candidates[index];
  const revisiting = index < furthest;
  const currentDecision = decisions[current.id]?.action;

  return (
    <div className="wizard-overlay">
      <div className="wizard wizard-wide">
        <div className="wizard-progress">
          Close out today · {index + 1} of {candidates.length}
          {revisiting ? " · revisiting" : ""}
          <span className="wizard-nav-hint"> — ↑↓ to browse decided items</span>
        </div>
        {current.breadcrumb.length > 0 && (
          <div className="wizard-breadcrumb">under: {[...current.breadcrumb].reverse().join(" > ")}</div>
        )}
        <div className="wizard-title">{current.title}</div>
        {current.stale && <div className="wizard-stale">⚠ {current.status.replace("_", " ")} for 3+ days</div>}
        {(current.status === "dropped" || current.status === "done") && (
          <div className="wizard-current-decision">
            Already marked {current.status} — pick what should happen to it now.
          </div>
        )}
        {revisiting && currentDecision && (
          <div className="wizard-current-decision">Currently: {SUMMARY_LABEL[currentDecision]}</div>
        )}

        {columns(current.id)}

        {showBacklogDate ? (
          <div className="wizard-backlog-date">
            <label>Surface again on (optional):</label>
            <input
              ref={dateInputRef}
              type="date"
              value={snoozeDate}
              onChange={(e) => setSnoozeDate(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") confirmBacklog();
                else if (e.key === "Escape") setShowBacklogDate(false);
              }}
            />
            <div className="wizard-actions">
              <button onClick={confirmBacklog}>Backlog{snoozeDate ? ` until ${snoozeDate}` : ""}</button>
              <button onClick={() => setShowBacklogDate(false)}>Cancel</button>
            </div>
          </div>
        ) : (
          <div className="wizard-actions">
            <button onClick={() => decide({ action: "carry_over" })}>[C] Carry over</button>
            <button onClick={() => setShowBacklogDate(true)}>[B] Backlog</button>
            <button onClick={() => decide({ action: "drop" })}>[D] Drop</button>
            <button onClick={() => decide({ action: "done" })}>[↵] Done</button>
            <button onClick={back} disabled={index === 0}>
              [⌫] Back
            </button>
            {revisiting && (
              <button onClick={forward}>[↓] Next</button>
            )}
          </div>
        )}
        <button className="wizard-cancel" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
