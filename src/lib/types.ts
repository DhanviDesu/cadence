export type Status =
  | "todo"
  | "wip"
  | "in_review"
  | "submitting"
  | "done"
  | "blocked"
  | "dropped";

export const STATUS_ORDER: Status[] = [
  "todo",
  "wip",
  "in_review",
  "submitting",
  "done",
  "blocked",
  "dropped",
];

export const STATUS_LABEL: Record<Status, string> = {
  todo: "Not Started",
  wip: "WIP",
  in_review: "In Review",
  submitting: "Submitting",
  done: "Done",
  blocked: "Blocked",
  dropped: "Dropped",
};

export interface TaskNode {
  id: number;
  parent_id: number | null;
  title: string;
  status: Status;
  status_override: boolean;
  note: string | null;
  children: TaskNode[];
}

export interface RoutineTaskNode {
  id: number;
  title: string;
  status: Status;
}

export interface RoutineSection {
  routine_id: number;
  name: string;
  after_tasks: boolean;
  tasks: RoutineTaskNode[];
}

export interface CarriedBanner {
  count: number;
  from_date: string;
}

export interface DayPayload {
  date: string;
  routines: RoutineSection[];
  tasks: TaskNode[];
  carried_banner: CarriedBanner | null;
}

export interface RoutineItemDef {
  id: number;
  title: string;
}

export interface RoutineDef {
  id: number;
  name: string;
  after_tasks: boolean;
  items: RoutineItemDef[];
}

export interface CloseOutCandidate {
  id: number;
  title: string;
  breadcrumb: string[];
  status: Status;
  stale: boolean;
}

export type CloseOutAction = "carry_over" | "drop" | "done" | "backlog";

export interface CloseOutDecision {
  action: CloseOutAction;
  snoozeUntil?: string;
}

export interface BacklogItem {
  id: number;
  title: string;
  status: Status;
  snooze_until: string | null;
  breadcrumb: string[];
}

export interface CloseOutResult {
  day: DayPayload;
  carried: number;
  dropped: number;
  done: number;
  backlogged: number;
}

export interface ImportSummary {
  historical_days: number;
  tasks_created: number;
  tasks_continued: number;
  carried_to_live: number;
}

export interface ExportAllSummary {
  count: number;
  directory: string;
}

export interface SearchResult {
  id: number;
  title: string;
  status: Status;
  note: string | null;
  breadcrumb: string[];
  dates: string[];
}

export interface RangeSummaryItem {
  id: number;
  title: string;
  status: Status;
  breadcrumb: string[];
}

export interface RangeSummary {
  start: string;
  end: string;
  completed: RangeSummaryItem[];
  dropped: RangeSummaryItem[];
  created: RangeSummaryItem[];
  still_active: RangeSummaryItem[];
}
