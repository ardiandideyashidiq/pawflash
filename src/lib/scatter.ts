import type { ScatterPartition } from "@/types/api";

export interface RegionGroup {
  region: string;
  parts: ScatterPartition[];
}

export function isFlashable(part: ScatterPartition): boolean {
  return part.is_download && part.file_name !== null && part.size > 0;
}

export function isBlocked(part: ScatterPartition): boolean {
  return (
    part.safety_class === "identity_or_calibration" ||
    part.safety_class === "dangerous"
  );
}

export function isSelectable(part: ScatterPartition): boolean {
  return isFlashable(part) && !isBlocked(part);
}

export function groupByRegion(parts: ScatterPartition[]): RegionGroup[] {
  const groups = new Map<string, ScatterPartition[]>();
  for (const part of parts) {
    const list = groups.get(part.region) ?? [];
    list.push(part);
    groups.set(part.region, list);
  }
  return [...groups.entries()]
    .map(([region, regionParts]) => ({
      region,
      parts: [...regionParts].sort(
        (a, b) => a.linear_start - b.linear_start || a.name.localeCompare(b.name),
      ),
    }))
    .sort(
      (a, b) =>
        minLinearStart(a.parts) - minLinearStart(b.parts) ||
        a.region.localeCompare(b.region),
    );
}

function minLinearStart(parts: ScatterPartition[]): number {
  return parts.reduce((min, p) => Math.min(min, p.linear_start), Infinity);
}

export type SafetyTone = "danger" | "identity" | "boot" | "muted";

export function safetyTone(part: ScatterPartition): SafetyTone {
  switch (part.safety_class) {
    case "dangerous":
      return "danger";
    case "identity_or_calibration":
      return "identity";
    case "bootloader_critical":
    case "boot_critical":
      return "boot";
    default:
      return "muted";
  }
}

export function safetyLabel(safetyClass: string): string {
  switch (safetyClass) {
    case "dangerous":
      return "Dangerous";
    case "identity_or_calibration":
      return "Identity / calibration";
    case "bootloader_critical":
      return "Bootloader critical";
    case "boot_critical":
      return "Boot critical";
    case "firmware":
      return "Firmware";
    case "android_system":
      return "System";
    case "regional":
      return "Regional";
    default:
      return "Unknown";
  }
}

export function formatHexAddr(value: number): string {
  return `0x${value.toString(16)}`;
}

export const SAFETY_LEGEND: { tone: SafetyTone; label: string }[] = [
  { tone: "danger", label: "Dangerous" },
  { tone: "identity", label: "Identity" },
  { tone: "boot", label: "Bootloader" },
];
