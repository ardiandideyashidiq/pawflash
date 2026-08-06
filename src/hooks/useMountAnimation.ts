import { useEffect, useState } from "react";

/**
 * Keeps an element mounted through an exit animation.
 *
 * - When `open` becomes true: mounts immediately, then flips `shown` on the
 *   next frames so the closed state paints before the enter transition runs.
 * - When `open` becomes false: flips `shown` off and keeps `mounted` true for
 *   `durationMs` so the exit transition completes before unmounting.
 */
export function useMountAnimation(open: boolean, durationMs: number) {
  const [mounted, setMounted] = useState(open);
  const [shown, setShown] = useState(open);

  useEffect(() => {
    if (open) {
      // Mount before the enter transition: the closed state must paint first,
      // so this synchronous flip is intentional (mount/unmount animation).
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setMounted(true);
      // Two frames: the first paints the closed state, the second triggers
      // the enter transition.
      const raf = requestAnimationFrame(() =>
        requestAnimationFrame(() => setShown(true)),
      );
      return () => cancelAnimationFrame(raf);
    }

    // Flip to the closing state synchronously so the exit transition runs
    // before the delayed unmount.
    setShown(false);
    const timeoutId = window.setTimeout(() => setMounted(false), durationMs);
    return () => window.clearTimeout(timeoutId);
  }, [open, durationMs]);

  return { mounted, shown };
}
