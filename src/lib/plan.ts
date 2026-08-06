import type { FlashPlanOptions } from "@/types/api";

/** Parent directory of an absolute path (handles `/` and `\`). */
function parentDir(path: string): string | null {
  if (!path) return null;
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (idx < 0) return null;
  return idx === 0 ? "/" : path.slice(0, idx);
}

export function buildFlashPlanOptions(
  exclude: string[],
  includePreloader: boolean,
  scatterPath: string,
): FlashPlanOptions {
  return {
    storage: "auto",
    exclude,
    firmware_dir: null,
    // Anchor resolution to the scatter's directory so the `..` containment
    // guard is active, mirroring the CLI's `package_root` behaviour.
    package_root: parentDir(scatterPath),
    image_verification: { check_images: false, image_search: false },
    allowance: { include_preloader: includePreloader, allow_incomplete_slots: false },
  };
}
