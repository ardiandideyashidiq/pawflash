import { Construction } from "lucide-react";

interface PlaceholderTabProps {
  title: string;
  description: string;
}

export function PlaceholderTab({ title, description }: PlaceholderTabProps) {
  return (
    <div className="flex min-h-full items-center justify-center p-6 text-center">
      <div className="flex flex-col items-center gap-3 rounded-md border border-border/80 bg-card px-8 py-7 shadow-[var(--panel-shadow)]">
        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
          <Construction className="h-6 w-6" />
        </div>
        <div className="space-y-1">
          <h3 className="text-lg font-bold tracking-tight text-foreground">
            {title}
          </h3>
          <p className="max-w-xs text-xs text-muted-foreground">
            {description}
          </p>
        </div>
      </div>
    </div>
  );
}
