export type Theme = "light" | "dark";

/** Error DTO returned by the Tauri backend (see `AppError` in src-tauri/src/lib.rs). */
export type AppError =
  | { kind: "NoDevice"; detail: { message: string } }
  | { kind: "Permission"; detail: { message: string } }
  | { kind: "Protocol"; detail: { message: string } }
  | { kind: "ActionFailed"; detail: { partition: string; message: string } }
  | { kind: "Cancelled"; detail: { message: string } }
  | { kind: "Timeout"; detail: { message: string } }
  | { kind: "Other"; detail: { message: string } };

/** Extract a readable message from a thrown value that may be an `AppError`
 * DTO, an `Error`, or a plain string. */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object") {
    if ("kind" in error) {
      const detail = (error as { detail?: { message?: string } }).detail;
      if (detail?.message) return detail.message;
      const kind = (error as { kind?: unknown }).kind;
      if (typeof kind === "string" && kind) return kind;
    }
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
    try {
      const json = JSON.stringify(error);
      if (json && json !== "{}") return json;
    } catch {
      return String(error);
    }
  }
  return String(error);
}

export interface DeviceInfo {
  connected: boolean;
  serial: string | null;
  vars: Record<string, string>;
}

export interface ScatterFile {
  path: string;
  format: string;
  text_hash: string;
  platform: string | null;
  project: string | null;
  general: unknown;
  layouts: Record<string, ScatterPartition[]>;
  warnings: string[];
  errors: string[];
}

export type StorageSelect = "auto" | "all" | "ufs" | "emmc";

export interface ScatterPartition {
  source: string;
  layout: string;
  index: string | null;
  name: string;
  file_name: string | null;
  is_download: boolean;
  type: string | null;
  linear_start: number;
  physical_start: number;
  size: number;
  region: string;
  storage: string | null;
  boundary_check: boolean;
  is_reserved: boolean;
  operation_type: string | null;
  is_upgradable: boolean | null;
  empty_boot_needed: boolean | null;
  combo_partsize_check: boolean | null;
  safety_class: string;
  raw: unknown;
}

export interface FlashPlanOptions {
  storage: StorageSelect;
  exclude: string[];
  firmware_dir: string | null;
  package_root: string | null;
  image_verification: { check_images: boolean; image_search: boolean };
  allowance: { include_preloader: boolean; allow_incomplete_slots: boolean };
}

export interface FlashOutcome {
  partition: string;
  success: boolean;
  response: string | null;
  duration: number;
  error: string | null;
}

export interface FlashResult {
  total: number;
  succeeded: number;
  failed: number;
  outcomes: FlashOutcome[];
  cancelled: boolean;
}

/** Raw `FlashPlan`/`FlashAction` DTOs serialized by the core planner. */
export interface PlanActionDto {
  action: string;
  partition: string;
  base_name: string;
  slot: string | null;
  layout: string;
  region: string;
  start: number;
  size: number;
  size_human: string;
  image: { path?: { resolved_path?: string | null } } | null;
  image_type: string | null;
  safety_class: string;
  reason: string;
  warnings: string[];
}

export interface FlashPlanDto {
  storage_selection: string;
  selected_layouts: string[];
  platform: string | null;
  project: string | null;
  firmware_dir: string | null;
  package_root: string | null;
  options: unknown;
  summary: {
    flash_count: number;
    skipped_count: number;
    missing_image_count: number;
    oversized_image_count: number;
    action_warning_count: number;
    incomplete_slot_base_count: number;
    warning_count: number;
    error_count: number;
  };
  actions: PlanActionDto[];
  skipped: Array<{ partition: string; reason: string }>;
  warnings: string[];
  errors: string[];
}

/** Frontend partition row derived from `PlanActionDto`. */
export interface PartitionRow {
  index: number;
  partition: string;
  action: string;
  size_human: string;
  image_path: string | null;
  image_name: string | null;
  image_type: string | null;
  region: string;
  selected: boolean;
}

/** Frontend flash-plan view (persisted across tab switches). */
export interface FlashPlanView {
  chipset: string | null;
  storage: string;
  project: string | null;
  rows: PartitionRow[];
  warnings: string[];
  errors: string[];
  flashCount: number;
  skippedCount: number;
}

