import { STATUS_LABEL } from "../lib/types";
import type { Status } from "../lib/types";

export function StatusChip({ status, derived }: { status: Status; derived: boolean }) {
  return (
    <span className={`status-chip status-${status}`} title={derived ? "derived from subtasks" : undefined}>
      {STATUS_LABEL[status]}
      {derived ? " ·" : ""}
    </span>
  );
}
