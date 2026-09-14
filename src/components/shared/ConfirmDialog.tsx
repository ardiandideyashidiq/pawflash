import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { createDismissibleDialogRootHandler } from "@/components/shared/dialogBehavior";

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  onConfirm: () => void | Promise<void>;
  confirmLabel?: string;
  destructive?: boolean;
  isPending?: boolean;
  isSuccess?: boolean;
  successMessage?: string;
  closeLabel?: string;
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  onConfirm,
  confirmLabel = "Continue",
  destructive = false,
  isPending = false,
  isSuccess = false,
  successMessage,
  closeLabel = "Close",
}: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={createDismissibleDialogRootHandler(onOpenChange)}>
      <DialogContent className="gap-4" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {isSuccess && successMessage ? (
            <DialogDescription className="max-w-[44ch] leading-6 font-medium text-foreground">
              {successMessage}
            </DialogDescription>
          ) : description ? (
            <DialogDescription className="max-w-[44ch] leading-6">{description}</DialogDescription>
          ) : null}
        </DialogHeader>
        <DialogFooter>
          {isSuccess ? (
            <Button
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              {closeLabel}
            </Button>
          ) : (
            <>
              <Button
                variant="outline"
                onClick={() => onOpenChange(false)}
              >
                {isPending ? "Abort" : "Cancel"}
              </Button>
              <Button
                variant={destructive ? "destructive" : "default"}
                onClick={onConfirm}
                disabled={isPending}
              >
                {isPending ? "Working..." : confirmLabel}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
