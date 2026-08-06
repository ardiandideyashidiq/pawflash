export type ProgressEvent =
  | { event: "Phase"; data: { phase: string; message: string } }
  | { event: "FlashProgress"; data: { partition: string; percent: number } }
  | {
      event: "Flashing";
      data: {
        partition: string;
        operation: string;
        bytes: number;
        total: number;
        overall_bytes: number;
        overall_total: number;
      };
    }
  | { event: "FlashComplete"; data: { partition: string; success: boolean; response: string | null } }
  | { event: "DeviceAction"; data: { action: string; detail: string } }
  | { event: "Overall"; data: { bytes: number; total: number } }
  | { event: "Warning"; data: { message: string } }
  | { event: "Error"; data: { message: string } }
  | { event: "Cancelled"; data: { message: string } }
  | { event: "ForceFastbootStage"; data: { stage: string; message: string } }
  | { event: "Done"; data: { ok: boolean; detail: string } };

export type ConsoleLevel = "info" | "success" | "error" | "warning" | "command" | "response";

export interface ConsoleEntry {
  id: number;
  /** Wall-clock timestamp in milliseconds. */
  timestamp: number;
  /** Precomputed `HH:MM:SS` label, formatted once at insert time. */
  time: string;
  text: string;
  level: ConsoleLevel;
}
