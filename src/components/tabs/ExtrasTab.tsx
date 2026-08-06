import { FastbootVars } from "@/components/extra-tab/FastbootVars";
import { ManualFlash } from "@/components/extra-tab/ManualFlash";

interface ExtrasTabProps {
  menuActionDisabled: boolean;
  isStartingFlash: boolean;
  onManualFlash: (partition: string, imagePath: string) => Promise<void>;
}

export default function ExtrasTab({
  menuActionDisabled,
  isStartingFlash,
  onManualFlash,
}: ExtrasTabProps) {
  return (
    <div className="flex min-h-full min-h-0 flex-col gap-5 lg:grid lg:grid-cols-2 lg:gap-6">
      <div className="flex flex-col gap-4">
        <ManualFlash
          disabled={menuActionDisabled}
          flashing={isStartingFlash}
          onManualFlash={onManualFlash}
        />
      </div>
      <div className="flex flex-col gap-4">
        <FastbootVars disabled={menuActionDisabled} className="flex-1 min-h-0" />
      </div>
    </div>
  );
}
