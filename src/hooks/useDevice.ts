import { useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeviceInfo } from "@/types/api";

export function useDevice() {
  const check = useCallback(() => invoke<DeviceInfo>("get_device_info"), []);
  const reboot = useCallback(
    (target: string) => invoke<void>("reboot_device", { target }),
    [],
  );
  const getVariable = useCallback(
    (name: string) => invoke<string>("get_var", { name }),
    [],
  );
  const setActiveSlot = useCallback(
    (slot: "a" | "b") => invoke<string>("set_active_slot", { slot }),
    [],
  );
  const unlockBootloader = useCallback(
    () => invoke<string>("unlock_bootloader"),
    [],
  );
  const lockBootloader = useCallback(
    () => invoke<string>("lock_bootloader"),
    [],
  );

  return useMemo(
    () => ({
      check,
      reboot,
      getVariable,
      setActiveSlot,
      unlockBootloader,
      lockBootloader,
    }),
    [check, reboot, getVariable, setActiveSlot, unlockBootloader, lockBootloader],
  );
}
