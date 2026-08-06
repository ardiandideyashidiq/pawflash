import type { SlotOverride } from "@/types/api";

export const slotLabel: Record<Exclude<SlotOverride, "">, string> = {
  a: "_a",
  b: "_b",
  active: "active slot",
  inactive: "inactive slot",
  all: "all slots",
};
