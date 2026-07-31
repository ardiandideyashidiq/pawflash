export type Theme = "light" | "dark";

export interface ConfirmAction {
  title: string;
  description: string;
  confirmLabel?: string;
  variant?: "destructive" | "default";
  onConfirm: () => void;
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

export type FlashMode = "dry-run" | "selective" | "dirty-flash";
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
  raw: unknown;
  unknown_fields: Record<string, unknown>;
}

export interface FlashPlanOptions {
  mode: FlashMode;
  storage: StorageSelect;
  parts: string[];
  groups: string[];
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
}

export interface PartitionRow {
  name: string;
  size: number;
  imageType: string | null;
  fileName: string | null;
  layout: string;
  flashable: boolean;
}
