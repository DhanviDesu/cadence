import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useOutliner } from "../hooks/useOutliner";
import { api } from "../lib/api";
import { formatDateLong } from "../lib/date";
import { TaskRow } from "./TaskRow";
import { RoutineBlock } from "./RoutineBlock";
import { CloseOutWizard } from "./CloseOutWizard";
import { RoutineManager } from "./RoutineManager";
import { HistoryView } from "./HistoryView";
import { BacklogView } from "./BacklogView";
import { ImportView } from "./ImportView";
import { OverflowMenu } from "./OverflowMenu";
import { SearchView } from "./SearchView";
import { SummaryView } from "./SummaryView";

export function DayView() {
  const { day, drafts, registerInput, handleKeyDown, setDraft, commitDraft, addFirstTask, replaceDay, lastDeleted, undo } =
    useOutliner();
  const [showCloseOut, setShowCloseOut] = useState(false);
  const [showRoutines, setShowRoutines] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [historyInitialDate, setHistoryInitialDate] = useState<string | undefined>(undefined);
  const [showBacklog, setShowBacklog] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [showSummary, setShowSummary] = useState(false);
  const [bannerDismissed, setBannerDismissed] = useState(false);
  const [carryingAll, setCarryingAll] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (showCloseOut || showRoutines || showHistory || showBacklog || showImport || showSearch || showSummary) {
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        setShowCloseOut(true);
      } else if ((e.metaKey || e.ctrlKey) && (e.key === "[" || e.key === "]")) {
        e.preventDefault();
        setHistoryInitialDate(undefined);
        setShowHistory(true);
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "b") {
        e.preventDefault();
        setShowBacklog(true);
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setShowSearch(true);
      } else if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "z") {
        // Every text input keeps its own native undo stack (same as Notes),
        // so only defer to it when the focused task field actually has a
        // live, uncommitted edit in progress (tracked in `drafts`) - not
        // just because focus happens to be in a field, since indent,
        // outdent, and new-task-creation all leave focus sitting in a field
        // afterward too, and those need ⌘Z to reach the structural undo
        // stack instead. ⌘⇧Z (redo) is left alone entirely.
        const active = document.activeElement as HTMLElement | null;
        const isTextField = active?.tagName === "INPUT" || active?.tagName === "TEXTAREA";
        const taskIdAttr = active?.getAttribute("data-task-id");
        const hasLiveDraft = taskIdAttr != null && drafts[Number(taskIdAttr)] !== undefined;
        const deferToNativeUndo = isTextField && (taskIdAttr == null || hasLiveDraft);
        if (!deferToNativeUndo) {
          e.preventDefault();
          undo();
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showCloseOut, showRoutines, showHistory, showBacklog, showImport, showSearch, showSummary, drafts, undo]);

  // The global-hotkey quick-add capture window runs as a separate webview and
  // creates the task via its own invoke call, so this window only learns
  // about it through this event instead of shared JS state.
  useEffect(() => {
    const unlisten = listen("task-added", () => {
      api.getToday().then(replaceDay);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [replaceDay]);

  if (!day) return <div className="day-view loading">Loading…</div>;

  const toggleRoutineTask = async (id: number) => {
    replaceDay(await api.toggleDone(id));
  };

  const carryEverything = async () => {
    if (carryingAll) return;
    setCarryingAll(true);
    const result = await api.runCloseOut({});
    replaceDay(result.day);
    setBannerDismissed(false);
    setToast(`Carried ${result.carried} task${result.carried === 1 ? "" : "s"} to ${result.day.date}.`);
    setCarryingAll(false);
  };

  const exportAll = async () => {
    if (exporting) return;
    setExporting(true);
    const result = await api.exportAllDays();
    setToast(`Exported ${result.count} day${result.count === 1 ? "" : "s"} to ${result.directory}.`);
    setExporting(false);
  };

  return (
    <div className="day-view">
      <div className="day-header">
        <h1>{formatDateLong(day.date)}</h1>
        <div className="day-actions">
          <button className="ghost-button" onClick={() => setShowCloseOut(true)}>
            Close Out (⌘⇧C)
          </button>
          <OverflowMenu
            items={[
              {
                label: "History (⌘[)",
                onClick: () => {
                  setHistoryInitialDate(undefined);
                  setShowHistory(true);
                },
              },
              { label: "Search (⌘F)", onClick: () => setShowSearch(true) },
              { label: "Summary", onClick: () => setShowSummary(true) },
              { label: "Backlog (⌘B)", onClick: () => setShowBacklog(true) },
              { label: "Routines", onClick: () => setShowRoutines(true) },
              { label: "Import", onClick: () => setShowImport(true) },
              { label: exporting ? "Exporting…" : "Export All", onClick: exportAll, disabled: exporting },
              {
                label: carryingAll ? "Carrying…" : "Carry All & Close",
                onClick: carryEverything,
                disabled: carryingAll,
              },
            ]}
          />
        </div>
      </div>

      {lastDeleted && (
        <div className="carried-banner">
          Deleted "{lastDeleted.task.title || "(untitled)"}".
          <button className="ghost-button" onClick={undo}>
            Undo (⌘Z)
          </button>
        </div>
      )}

      {toast && (
        <div className="carried-banner">
          {toast}
          <button className="ghost-button" onClick={() => setToast(null)}>
            Dismiss
          </button>
        </div>
      )}

      {day.carried_banner && !bannerDismissed && (
        <div className="carried-banner">
          Carried {day.carried_banner.count} task{day.carried_banner.count === 1 ? "" : "s"} from{" "}
          {day.carried_banner.from_date}. Review?
          <button className="ghost-button" onClick={() => setBannerDismissed(true)}>
            Dismiss
          </button>
        </div>
      )}

      {day.routines
        .filter((section) => !section.after_tasks)
        .map((section) => (
          <RoutineBlock key={section.routine_id} section={section} onToggle={toggleRoutineTask} />
        ))}

      {day.tasks.length === 0 ? (
        <button className="add-first" onClick={() => addFirstTask()}>
          + Add your first task
        </button>
      ) : (
        <div className="task-list">
          {day.tasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              depth={0}
              registerInput={registerInput}
              onChange={setDraft}
              onKeyDown={handleKeyDown}
              onBlur={commitDraft}
            />
          ))}
        </div>
      )}

      {day.routines.some((section) => section.after_tasks) && <hr className="routines-after-divider" />}
      {day.routines
        .filter((section) => section.after_tasks)
        .map((section) => (
          <RoutineBlock key={section.routine_id} section={section} onToggle={toggleRoutineTask} />
        ))}

      {showCloseOut && (
        <CloseOutWizard
          today={day.tasks}
          todayDate={day.date}
          onCancel={() => setShowCloseOut(false)}
          onDone={(result) => {
            replaceDay(result.day);
            setShowCloseOut(false);
            setBannerDismissed(false);
          }}
        />
      )}

      {showRoutines && (
        <RoutineManager onClose={() => setShowRoutines(false)} onChange={() => api.getToday().then(replaceDay)} />
      )}

      {showHistory && (
        <HistoryView
          liveDate={day.date}
          initialDate={historyInitialDate}
          onClose={() => setShowHistory(false)}
        />
      )}

      {showSearch && (
        <SearchView
          liveDate={day.date}
          onClose={() => setShowSearch(false)}
          onJumpToDate={(date) => {
            setHistoryInitialDate(date);
            setShowHistory(true);
          }}
        />
      )}

      {showBacklog && (
        <BacklogView
          onClose={() => setShowBacklog(false)}
          onPulled={(pulledDay) => {
            replaceDay(pulledDay);
            setShowBacklog(false);
          }}
        />
      )}

      {showImport && (
        <ImportView
          onClose={() => setShowImport(false)}
          onImported={(importedDay) => {
            replaceDay(importedDay);
          }}
        />
      )}

      {showSummary && <SummaryView liveDate={day.date} onClose={() => setShowSummary(false)} />}
    </div>
  );
}
