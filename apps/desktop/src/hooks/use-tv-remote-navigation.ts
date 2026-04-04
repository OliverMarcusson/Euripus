import { useEffect } from "react";
import { useTvModeStore } from "@/store/tv-mode-store";

const TV_FOCUSABLE_SELECTOR = [
  "[data-tv-focusable='true']",
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "[role='button']",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

type Direction = "left" | "right" | "up" | "down";

export function useTvRemoteNavigation() {
  const isTvMode = useTvModeStore((state) => state.isTvMode);

  useEffect(() => {
    if (!isTvMode) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }

      const activeElement = document.activeElement as HTMLElement | null;

      if (event.key === "Escape" || event.key === "Backspace") {
        if (isEditable(activeElement)) {
          return;
        }

        event.preventDefault();
        window.history.back();
        return;
      }

      const direction = mapDirection(event.key);
      if (!direction) {
        return;
      }

      if (isEditable(activeElement)) {
        return;
      }

      event.preventDefault();

      const target = findNextFocusable(direction, activeElement);
      if (target) {
        target.focus();
        target.scrollIntoView({ block: "nearest", inline: "nearest" });
        return;
      }

      scrollNearestContainer(activeElement, direction);
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isTvMode]);
}

function mapDirection(key: string): Direction | null {
  if (key === "ArrowLeft") {
    return "left";
  }

  if (key === "ArrowRight") {
    return "right";
  }

  if (key === "ArrowUp") {
    return "up";
  }

  if (key === "ArrowDown") {
    return "down";
  }

  return null;
}

function isEditable(element: HTMLElement | null) {
  if (!element) {
    return false;
  }

  const tagName = element.tagName.toLowerCase();
  return (
    tagName === "input" ||
    tagName === "textarea" ||
    tagName === "select" ||
    element.isContentEditable
  );
}

function getFocusableElements() {
  return Array.from(document.querySelectorAll<HTMLElement>(TV_FOCUSABLE_SELECTOR)).filter(
    (element) =>
      !element.matches(":disabled,[aria-hidden='true']") &&
      element.tabIndex !== -1 &&
      isVisible(element),
  );
}

function isVisible(element: HTMLElement) {
  const rect = element.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return false;
  }

  const styles = window.getComputedStyle(element);
  return styles.visibility !== "hidden" && styles.display !== "none";
}

function focusFallback() {
  const autofocusTarget = document.querySelector<HTMLElement>("[data-tv-autofocus='true']");
  if (autofocusTarget && isVisible(autofocusTarget)) {
    return autofocusTarget;
  }

  return getFocusableElements()[0] ?? null;
}

function findNextFocusable(direction: Direction, activeElement: HTMLElement | null) {
  const focusableElements = getFocusableElements();
  if (!focusableElements.length) {
    return null;
  }

  if (!activeElement || !focusableElements.includes(activeElement)) {
    return focusFallback();
  }

  const currentRect = activeElement.getBoundingClientRect();
  const currentCenterX = currentRect.left + currentRect.width / 2;
  const currentCenterY = currentRect.top + currentRect.height / 2;

  let bestCandidate: HTMLElement | null = null;
  let bestScore = Number.POSITIVE_INFINITY;

  for (const candidate of focusableElements) {
    if (candidate === activeElement) {
      continue;
    }

    const rect = candidate.getBoundingClientRect();
    const centerX = rect.left + rect.width / 2;
    const centerY = rect.top + rect.height / 2;
    const deltaX = centerX - currentCenterX;
    const deltaY = centerY - currentCenterY;

    if (direction === "left" && deltaX >= -4) {
      continue;
    }

    if (direction === "right" && deltaX <= 4) {
      continue;
    }

    if (direction === "up" && deltaY >= -4) {
      continue;
    }

    if (direction === "down" && deltaY <= 4) {
      continue;
    }

    const primaryDistance = direction === "left" || direction === "right" ? Math.abs(deltaX) : Math.abs(deltaY);
    const secondaryDistance = direction === "left" || direction === "right" ? Math.abs(deltaY) : Math.abs(deltaX);
    const score = primaryDistance * primaryDistance + secondaryDistance * secondaryDistance * 3;

    if (score < bestScore) {
      bestScore = score;
      bestCandidate = candidate;
    }
  }

  return bestCandidate;
}

function scrollNearestContainer(activeElement: HTMLElement | null, direction: Direction) {
  const scrollRoot = findScrollableParent(activeElement) ?? document.scrollingElement;
  if (!scrollRoot) {
    return;
  }

  const delta = direction === "up" ? -120 : direction === "down" ? 120 : 0;
  if (!delta) {
    return;
  }

  scrollRoot.scrollBy({ top: delta, behavior: "smooth" });
}

function findScrollableParent(activeElement: HTMLElement | null) {
  let current = activeElement?.parentElement ?? null;

  while (current) {
    const { overflowY } = window.getComputedStyle(current);
    const canScroll = (overflowY === "auto" || overflowY === "scroll") && current.scrollHeight > current.clientHeight;
    if (canScroll) {
      return current;
    }

    current = current.parentElement;
  }

  return null;
}
