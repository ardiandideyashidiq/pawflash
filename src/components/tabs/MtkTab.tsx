import { Construction } from "lucide-react";

export default function MtkTab() {
  return (
    <div className="flex min-h-full items-center justify-center p-6 text-center">
      <div className="flex flex-col items-center gap-3 rounded-lg border border-primary/30 bg-background/80 px-8 py-7 shadow-2xl backdrop-blur-md">
        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
          <Construction className="h-6 w-6" />
        </div>
        <div className="space-y-1">
          <h3 className="text-lg font-bold tracking-tight text-foreground">
            Work In Progress
          </h3>
          <p className="max-w-xs text-xs text-muted-foreground">
            The MTK Client bridge tab is currently under active development.
          </p>
        </div>
      </div>
    </div>
  );
}
