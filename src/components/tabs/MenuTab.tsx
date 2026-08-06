import { DeviceSection } from "@/components/menu-tab/DeviceSection";
import { BootloaderSection } from "@/components/menu-tab/BootloaderSection";
import { SlotSection } from "@/components/menu-tab/SlotSection";

interface MenuTabProps {
  onForceFastboot: () => void;
  menuActionDisabled: boolean;
}

export default function MenuTab({ onForceFastboot, menuActionDisabled }: MenuTabProps) {
  return (
    <div className="flex min-h-full min-h-0 flex-col gap-3 lg:grid lg:grid-cols-2 lg:gap-4">
      <div className="flex flex-col gap-3">
        <DeviceSection
          onForceFastboot={onForceFastboot}
          forceFastbootDisabled={menuActionDisabled}
          disableVbmetaDisabled={menuActionDisabled}
          disabled={menuActionDisabled}
        />
        <BootloaderSection disabled={menuActionDisabled} />
      </div>
      <div className="flex flex-col gap-3">
        <SlotSection disabled={menuActionDisabled} />
      </div>
    </div>
  );
}
