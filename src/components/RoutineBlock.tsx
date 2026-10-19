import { useState } from "react";
import type { RoutineSection } from "../lib/types";

interface Props {
  section: RoutineSection;
  onToggle: (taskId: number) => void;
}

export function RoutineBlock({ section, onToggle }: Props) {
  const [collapsed, setCollapsed] = useState(false);
  const doneCount = section.tasks.filter((t) => t.status === "done").length;

  return (
    <div className="routine-block">
      <button className="routine-header" onClick={() => setCollapsed((c) => !c)}>
        <span className={`chevron${collapsed ? " collapsed" : ""}`}>▸</span>
        {section.name}
        <span className="routine-count">
          {doneCount}/{section.tasks.length}
        </span>
      </button>
      {!collapsed && (
        <div className="routine-items">
          {section.tasks.map((t) => (
            <label key={t.id} className="routine-item">
              <input type="checkbox" checked={t.status === "done"} onChange={() => onToggle(t.id)} />
              <span className={t.status === "done" ? "done-text" : ""}>{t.title}</span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
