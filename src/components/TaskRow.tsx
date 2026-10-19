import { useRef } from "react";
import type { TaskNode } from "../lib/types";
import { StatusChip } from "./StatusChip";

interface Props {
  task: TaskNode;
  depth: number;
  registerInput: (id: number, el: HTMLInputElement | null) => void;
  onChange: (id: number, value: string) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>, task: TaskNode) => void;
  onBlur: (task: TaskNode) => Promise<string>;
}

function rollup(task: TaskNode): [number, number] {
  let done = 0;
  for (const c of task.children) if (c.status === "done") done++;
  return [done, task.children.length];
}

export function TaskRow({ task, depth, registerInput, onChange, onKeyDown, onBlur }: Props) {
  const isDerived = task.children.length > 0 && !task.status_override;
  const [done, total] = rollup(task);
  const inputRef = useRef<HTMLInputElement | null>(null);

  return (
    <div className="task-row-group">
      <div className="task-row" style={{ paddingLeft: `${depth * 24}px` }}>
        <span className={`bullet status-dot-${task.status}`} />
        <input
          ref={(el) => {
            inputRef.current = el;
            registerInput(task.id, el);
          }}
          // Lets the window-level ⌘Z handler tell "focus is in this task's
          // field" apart from "there's an uncommitted edit in it" - see
          // DayView, which only defers to native text-undo in the latter
          // case, otherwise indent/outdent/create would never be undoable
          // since focus always lands back in a field after those.
          data-task-id={task.id}
          className="task-input"
          // Deliberately uncontrolled (defaultValue, not value): a
          // controlled input means React sets the DOM value on every
          // keystroke, which silently clears the browser's native per-field
          // undo stack. The DOM owns the text while you type; we only ever
          // reach in and overwrite it once, right after a commit resolves,
          // via the ref below - never during normal typing.
          defaultValue={task.title}
          onChange={(e) => onChange(task.id, e.target.value)}
          onBlur={async () => {
            const finalTitle = await onBlur(task);
            if (inputRef.current) inputRef.current.value = finalTitle;
          }}
          onKeyDown={(e) => onKeyDown(e, task)}
        />
        {total > 0 && (
          <span className="rollup">
            {done}/{total}
          </span>
        )}
        <StatusChip status={task.status} derived={isDerived} />
      </div>
      {task.note && (
        <div className="task-note" style={{ paddingLeft: `${depth * 24 + 20}px` }}>
          {task.note}
        </div>
      )}
      {task.children.map((child) => (
        <TaskRow
          key={child.id}
          task={child}
          depth={depth + 1}
          registerInput={registerInput}
          onChange={onChange}
          onKeyDown={onKeyDown}
          onBlur={onBlur}
        />
      ))}
    </div>
  );
}
