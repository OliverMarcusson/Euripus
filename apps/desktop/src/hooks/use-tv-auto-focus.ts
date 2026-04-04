import { useEffect } from "react";
import { useTvModeStore } from "@/store/tv-mode-store";

export function useTvAutoFocus(selector: string | null, deps: ReadonlyArray<unknown> = []) {
  const isTvMode = useTvModeStore((state) => state.isTvMode);

  useEffect(() => {
    if (!isTvMode || !selector) {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      const target = document.querySelector<HTMLElement>(selector);
      if (!target || target.matches(":disabled")) {
        return;
      }

      target.focus();
    });

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [isTvMode, selector, ...deps]);
}
