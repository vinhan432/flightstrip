// One place for the global key map. Keystrokes are ignored while focus is in
// a text field or a control that handles its own keys.

import { useEffect } from "react";

export type KeyMap = Record<string, (event: KeyboardEvent) => void>;

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}

export function useKeyboard(map: KeyMap, enabled = true) {
  useEffect(() => {
    if (!enabled) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (isTypingTarget(event.target)) return;

      // Normalise so Space and arrows have stable names.
      const key = event.key;
      const handler = map[key];
      if (!handler) return;

      handler(event);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [map, enabled]);
}
