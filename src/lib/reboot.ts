import { Cpu, ShieldAlert, Smartphone, Zap, type LucideIcon } from "lucide-react";

export type RebootTarget = "system" | "bootloader" | "fastbootd" | "recovery";

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
};
