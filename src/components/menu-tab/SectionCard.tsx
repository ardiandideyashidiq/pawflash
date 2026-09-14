import type { ReactNode } from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const sectionCardVariants = cva("", {
  variants: {
    variant: {
      default: "panel-shell p-4 md:p-5",
      flat: "rounded-md border border-border/60 bg-card/60 p-4 md:p-5",
    },
  },
  defaultVariants: {
    variant: "default",
  },
});

interface SectionCardProps extends VariantProps<typeof sectionCardVariants> {
  title: string;
  children: ReactNode;
  className?: string;
  contentClassName?: string;
}

export function SectionCard({
  title,
  children,
  className,
  contentClassName,
  variant,
}: SectionCardProps) {
  return (
    <section className={cn(sectionCardVariants({ variant }), className)}>
      <h3 className="shrink-0 text-sm font-semibold tracking-[0.04em] text-foreground">{title}</h3>
      <div className={cn("mt-4", contentClassName)}>{children}</div>
    </section>
  );
}
