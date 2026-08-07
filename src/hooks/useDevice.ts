import { useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeviceInfo } from "@/types/api";
import { useSimulation } from "@/hooks/useSimulation";

export function useDevice() {
  const { simulate } = useSimulation();

  const check = useCallback(
    () => invoke<DeviceInfo>("get_device_info", { simulate }),
    [simulate],
  );
  const reboot = useCallback(
    (target: string) => invoke<void>("reboot_device", { target, simulate }),
    [simulate],
  );
  const getVariable = useCallback(
    (name: string) => invoke<string>("get_var", { name, simulate }),
    [simulate],
  );
  const setActiveSlot = useCallback(
    (slot: "a" | "b") => invoke<string>("set_active_slot", { slot, simulate }),
    [simulate],
  );
  const unlockBootloader = useCallback(
    () => invoke<string>("unlock_bootloader", { simulate }),
    [simulate],
  );
  const lockBootloader = useCallback(
    () => invoke<string>("lock_bootloader", { simulate }),
    [simulate],
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
