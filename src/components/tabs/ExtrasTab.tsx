import { DeviceSection } from "@/components/menu-tab/DeviceSection";
import { BootloaderSection } from "@/components/menu-tab/BootloaderSection";
import { SlotSection } from "@/components/menu-tab/SlotSection";
import { ManualFlash } from "@/components/extra-tab/ManualFlash";
import { FastbootVars } from "@/components/extra-tab/FastbootVars";

interface ExtrasTabProps {
  onForceFastboot: () => void;
  menuActionDisabled: boolean;
  isStartingFlash: boolean;
  onManualFlash: (partition: string, imagePath: string) => Promise<void>;
}

export default function ExtrasTab({
  onForceFastboot,
  menuActionDisabled,
  isStartingFlash,
  onManualFlash,
}: ExtrasTabProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 lg:grid lg:grid-cols-2 lg:gap-4">
      <div className="flex flex-col gap-3">
        <DeviceSection
          onForceFastboot={onForceFastboot}
          forceFastbootDisabled={menuActionDisabled}
          disableVbmetaDisabled={menuActionDisabled}
          disabled={menuActionDisabled}
        />
        <BootloaderSection disabled={menuActionDisabled} />
        <SlotSection disabled={menuActionDisabled} />
        <ManualFlash
          disabled={menuActionDisabled}
          flashing={isStartingFlash}
          onManualFlash={onManualFlash}
        />
      </div>
      <div className="flex min-h-[300px] flex-1 flex-col">
        <FastbootVars disabled={menuActionDisabled} className="h-full flex-1 min-h-0" />
      </div>
    </div>
  );
}

