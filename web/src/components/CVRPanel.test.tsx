// The CVR drawer: it must explain the selected step, name the prompt that
// caused it, and let a reader jump to that prompt. The focus trap is the
// part that is easy to get wrong, so it is covered here.

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CVRPanel } from "./CVRPanel";
import type { Session, Incident } from "../types";
import risky from "../../../fixtures/risky-session.json";

const session = risky as unknown as Session;

/** The first file_change that carries a prompt id AND real diff lines. */
const changeIndex = session.steps.findIndex(
  (s) =>
    s.kind === "file_change" &&
    s.prompt_id &&
    Array.isArray(s.hunks) &&
    s.hunks.some((h) => h.lines.length > 0),
);

function open(stepIndex: number, incident: Incident | null = null) {
  const onClose = vi.fn();
  const onJump = vi.fn();
  const utils = render(
    <CVRPanel
      session={session}
      stepIndex={stepIndex}
      incident={incident}
      onClose={onClose}
      onJump={onJump}
    />,
  );
  return { ...utils, onClose, onJump };
}

describe("CVRPanel", () => {
  it("names the prompt that caused a file change", () => {
    open(changeIndex);
    expect(screen.getByText(/caused by prompt/i)).toBeTruthy();
  });

  it("jumps to the prompt's step when asked", () => {
    const { onJump } = open(changeIndex);
    fireEvent.click(screen.getByText(/jump to the prompt/i));

    const step = session.steps[changeIndex] as { prompt_id?: string };
    const prompt = session.prompts.find((p) => p.id === step.prompt_id);
    expect(onJump).toHaveBeenCalledWith(prompt!.step);
  });

  it("renders the diff for a file change", () => {
    const { container } = open(changeIndex);
    expect(container.querySelector("table.diff")).not.toBeNull();
  });

  it("shows an incident's evidence and why it matters", () => {
    const incident = session.incidents[0];
    open(incident.step, incident);
    expect(screen.getByText(incident.evidence)).toBeTruthy();
    expect(screen.getByText(incident.why_it_matters)).toBeTruthy();
    // Severity is text, not only colour.
    expect(screen.getByText(incident.severity)).toBeTruthy();
  });

  it("lists the surrounding steps and jumps on click", () => {
    const { onJump, container } = open(changeIndex);
    const rows = container.querySelectorAll(".cvr-transcript button");
    expect(rows.length).toBeGreaterThan(1);

    fireEvent.click(rows[0]);
    expect(onJump).toHaveBeenCalled();
  });

  it("closes on Escape", () => {
    const { onClose } = open(changeIndex);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("moves focus into the panel when it opens", () => {
    open(changeIndex);
    expect(document.activeElement?.getAttribute("aria-label")).toBe(
      "Step detail",
    );
  });

  it("returns focus to the opener when it closes", () => {
    // A real opener, focused before the drawer mounts.
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();

    const { unmount } = open(changeIndex);
    unmount();

    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});
