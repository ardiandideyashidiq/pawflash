import { Cpu, Power, Radio, ShieldAlert, Smartphone, TestTube, Zap, type LucideIcon } from "lucide-react";

export type FastbootRebootTarget = "system" | "bootloader" | "fastbootd" | "recovery";

export type MtkRebootTarget = "mtk:normal" | "mtk:fastboot" | "mtk:recovery" | "mtk:meta" | "mtk:test";

export type RebootTarget = FastbootRebootTarget | MtkRebootTarget | "shutdown";

export interface RebootTargetMeta {
  label: string;
  description: string;
  icon: LucideIcon;
  iconColor: string;
}

export const rebootTargets: RebootTarget[] = [
  "system",
  "bootloader",
  "fastbootd",
  "recovery",
  "shutdown",
];

export const mtkRebootTargets: RebootTarget[] = [
  "mtk:normal",
  "mtk:fastboot",
  "mtk:recovery",
  "mtk:meta",
  "mtk:test",
  "shutdown",
];

export const targetMeta: Record<RebootTarget, RebootTargetMeta> = {
  system: {
    label: "System",
    description: "Reboot normally to Android OS",
    icon: Smartphone,
    iconColor: "text-emerald-400",
  },
  bootloader: {
    label: "Bootloader",
    description: "Reboot into Fastboot BL mode",
    icon: Cpu,
    iconColor: "text-amber-400",
  },
  fastbootd: {
    label: "Fastbootd",
    description: "Reboot into Userspace Fastboot",
    icon: Zap,
    iconColor: "text-trace-copper",
  },
  recovery: {
    label: "Recovery",
    description: "Reboot into Android Recovery",
    icon: ShieldAlert,
    iconColor: "text-rose-400",
  },
  "mtk:normal": {
    label: "Normal Boot",
    description: "Boot into Android OS",
    icon: Smartphone,
    iconColor: "text-emerald-400",
  },
  "mtk:fastboot": {
    label: "Fastboot Mode",
    description: "Boot to Fastboot mode",
    icon: Cpu,
    iconColor: "text-amber-400",
  },
  "mtk:recovery": {
    label: "Recovery Mode",
    description: "Boot to Android Recovery",
    icon: ShieldAlert,
    iconColor: "text-rose-400",
  },
  "mtk:meta": {
    label: "Meta Mode",
    description: "MediaTek calibration / RF mode",
    icon: Radio,
    iconColor: "text-trace-copper",
  },
  "mtk:test": {
    label: "Factory / Test",
    description: "Factory test diagnostics mode",
    icon: TestTube,
    iconColor: "text-signal-amber",
  },
  shutdown: {
    label: "Shutdown",
    description: "Power off device completely",
    icon: Power,
    iconColor: "text-error",
  },
};
