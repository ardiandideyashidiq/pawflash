import type { FlashPlanOptions } from "@/types/api";

export function buildFlashPlanOptions(
  exclude: string[],
  includePreloader: boolean,
): FlashPlanOptions {
  return {
    storage: "auto",
    exclude,
    firmware_dir: null,
    package_root: null,
    image_verification: { check_images: false, image_search: false },
    allowance: { include_preloader: includePreloader, allow_incomplete_slots: false },
    clean: "no",
  };
}
