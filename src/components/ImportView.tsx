import { useRef, useState } from "react";
import { api } from "../lib/api";
import type { DayPayload, ImportSummary } from "../lib/types";

interface Props {
  onClose: () => void;
  onImported: (day: DayPayload) => void;
}

export function ImportView({ onClose, onImported }: Props) {
  const [text, setText] = useState("");
  const [importing, setImporting] = useState(false);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const loadFile = (file: File) => {
    const reader = new FileReader();
    reader.onload = () => {
      const loaded = String(reader.result ?? "");
      // A file load is a deliberate bulk replace, not normal typing, so
      // overwriting both the DOM value and the tracked state here is fine.
      if (textareaRef.current) textareaRef.current.value = loaded;
      setText(loaded);
    };
    reader.readAsText(file);
  };

  const runImport = async () => {
    const current = textareaRef.current?.value ?? text;
    if (!current.trim() || importing) return;
    setImporting(true);
    setError(null);
    try {
      const result = await api.importNote(current);
      setSummary(result);
      const day = await api.getToday();
      onImported(day);
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="wizard-overlay">
      <div className="wizard wizard-wide import-view">
        <div className="wizard-progress">Import from Apple Note</div>

        {summary ? (
          <>
            <div className="wizard-title">Import complete</div>
            <ul className="import-summary">
              <li>{summary.historical_days} historical day(s) added</li>
              <li>{summary.tasks_created} new task(s) created</li>
              <li>{summary.tasks_continued} continued an existing task across days</li>
              <li>{summary.carried_to_live} unfinished item(s) carried onto today</li>
            </ul>
            <button className="wizard-cancel" onClick={onClose}>
              Done
            </button>
          </>
        ) : (
          <>
            <p className="import-hint">
              Paste your dumped daily note below, or load it from a file. Day headers like
              "09/22 TODAY" or "09/21" split it into days; nested "-" bullets become subtasks.
              Everything except the most recent day imports as history - the most recent day's
              unfinished items carry onto today, the same way closing out normally works.
            </p>
            <div className="import-file-row">
              <button className="ghost-button" onClick={() => fileInputRef.current?.click()}>
                Load from file…
              </button>
              <input
                ref={fileInputRef}
                type="file"
                accept=".txt,.md,text/plain"
                hidden
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) loadFile(file);
                  e.target.value = "";
                }}
              />
            </div>
            <textarea
              ref={textareaRef}
              className="import-textarea"
              placeholder="Paste note content here…"
              // Uncontrolled: `onChange` still updates `text` for the
              // disabled-button check, it just never feeds back into
              // `value`, so native undo survives large pastes/edits.
              defaultValue=""
              onChange={(e) => setText(e.target.value)}
            />
            {error && <div className="wizard-stale">{error}</div>}
            <div className="wizard-actions">
              <button onClick={runImport} disabled={importing || !text.trim()}>
                {importing ? "Importing…" : "Import"}
              </button>
            </div>
            <button className="wizard-cancel" onClick={onClose}>
              Cancel
            </button>
          </>
        )}
      </div>
    </div>
  );
}
