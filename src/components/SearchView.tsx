import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { SearchResult } from "../lib/types";
import { StatusChip } from "./StatusChip";

interface Props {
  liveDate: string;
  onClose: () => void;
  onJumpToDate: (date: string) => void;
}

export function SearchView({ liveDate, onClose, onJumpToDate }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!query.trim()) {
      setResults(null);
      return;
    }
    const handle = setTimeout(() => {
      api.searchTasks(query).then(setResults);
    }, 120);
    return () => clearTimeout(handle);
  }, [query]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div className="wizard-overlay">
      <div className="wizard wizard-wide search-view">
        <div className="wizard-progress">Search</div>
        <input
          ref={inputRef}
          className="search-input"
          placeholder="Search titles and notes across every day…"
          // Uncontrolled: `onChange` still drives the debounced search below,
          // but nothing feeds back into `value`, so the DOM keeps native undo.
          defaultValue=""
          onChange={(e) => setQuery(e.target.value)}
        />

        <div className="search-results">
          {results === null ? (
            query.trim() ? null : <div className="preview-empty">Start typing to search.</div>
          ) : results.length === 0 ? (
            <div className="preview-empty">No matches.</div>
          ) : (
            results.map((r) => (
              <div key={r.id} className="search-result">
                <div className="search-result-main">
                  {r.breadcrumb.length > 0 && (
                    <div className="wizard-breadcrumb">under: {[...r.breadcrumb].reverse().join(" > ")}</div>
                  )}
                  <div className="search-result-title">
                    {r.title || "(untitled)"}
                    <StatusChip status={r.status} derived={false} />
                  </div>
                  {r.note && <div className="search-result-note">{r.note}</div>}
                </div>
                <div className="search-result-dates">
                  {r.dates.map((d) => (
                    <button
                      key={d}
                      className="ghost-button"
                      onClick={() => {
                        onJumpToDate(d);
                        onClose();
                      }}
                    >
                      {d === liveDate ? "Today" : d}
                    </button>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>

        <button className="wizard-cancel" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
