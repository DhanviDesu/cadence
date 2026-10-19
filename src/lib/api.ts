import { invoke } from "@tauri-apps/api/core";
import type {
  BacklogItem,
  CloseOutCandidate,
  CloseOutDecision,
  CloseOutResult,
  DayPayload,
  ExportAllSummary,
  ImportSummary,
  RangeSummary,
  RoutineDef,
  SearchResult,
  Status,
} from "./types";

export const api = {
  getToday: () => invoke<DayPayload>("get_today"),

  createTask: (parentId: number | null, afterId: number | null, title: string) =>
    invoke<DayPayload>("create_task", { parentId, afterId, title }),

  updateTaskTitle: (id: number, title: string) =>
    invoke<DayPayload>("update_task_title", { id, title }),

  setStatus: (id: number, status: Status) =>
    invoke<DayPayload>("set_status", { id, status }),

  clearStatusOverride: (id: number) =>
    invoke<DayPayload>("clear_status_override", { id }),

  toggleDone: (id: number) => invoke<DayPayload>("toggle_done", { id }),

  deleteTask: (id: number) => invoke<DayPayload>("delete_task", { id }),

  indentTask: (id: number) => invoke<DayPayload>("indent_task", { id }),

  outdentTask: (id: number) => invoke<DayPayload>("outdent_task", { id }),

  moveTask: (id: number, direction: "up" | "down") =>
    invoke<DayPayload>("move_task", { id, direction }),

  listRoutines: () => invoke<RoutineDef[]>("list_routines"),

  createRoutine: (name: string) => invoke<RoutineDef[]>("create_routine", { name }),

  deleteRoutine: (routineId: number) => invoke<RoutineDef[]>("delete_routine", { routineId }),

  addRoutineItem: (routineId: number, title: string) =>
    invoke<RoutineDef[]>("add_routine_item", { routineId, title }),

  updateRoutineItem: (itemId: number, title: string) =>
    invoke<RoutineDef[]>("update_routine_item", { itemId, title }),

  deleteRoutineItem: (itemId: number) => invoke<RoutineDef[]>("delete_routine_item", { itemId }),

  moveRoutine: (routineId: number, direction: "up" | "down") =>
    invoke<RoutineDef[]>("move_routine", { routineId, direction }),

  moveRoutineItem: (itemId: number, direction: "up" | "down") =>
    invoke<RoutineDef[]>("move_routine_item", { itemId, direction }),

  setRoutineAfterTasks: (routineId: number, afterTasks: boolean) =>
    invoke<RoutineDef[]>("set_routine_after_tasks", { routineId, afterTasks }),

  getDayByDate: (date: string) => invoke<DayPayload>("get_day_by_date", { date }),

  importNote: (text: string) => invoke<ImportSummary>("import_note", { text }),

  exportAllDays: () => invoke<ExportAllSummary>("export_all_days"),

  searchTasks: (query: string) => invoke<SearchResult[]>("search_tasks", { query }),

  summarizeRange: (start: string, end: string) => invoke<RangeSummary>("summarize_range", { start, end }),

  getCloseOutCandidates: () => invoke<CloseOutCandidate[]>("get_close_out_candidates"),

  runCloseOut: (decisions: Record<number, CloseOutDecision>) =>
    invoke<CloseOutResult>("run_close_out", { decisions }),

  getBacklog: () => invoke<BacklogItem[]>("get_backlog"),

  pullFromBacklog: (id: number) => invoke<DayPayload>("pull_from_backlog", { id }),

  setBacklogSnooze: (id: number, snoozeUntil: string | null) =>
    invoke<BacklogItem[]>("set_backlog_snooze", { id, snoozeUntil }),

  dropFromBacklog: (id: number) => invoke<BacklogItem[]>("drop_from_backlog", { id }),
};
