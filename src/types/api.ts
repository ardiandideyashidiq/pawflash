export type Theme = "light" | "dark";

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
export type CleanMode = "no" | "yes";

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
  clean: CleanMode;
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

