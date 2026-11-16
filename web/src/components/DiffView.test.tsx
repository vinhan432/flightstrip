import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { DiffView } from "./DiffView";
import type { Session } from "../types";
import weakened from "../../../fixtures/weakened-tests.json";

const session = weakened as unknown as Session;

describe("DiffView", () => {
  it("shows added and removed lines", () => {
    const step = session.steps[3]; // the src/upload.ts change
    render(<DiffView hunks={step.hunks} />);
    expect(screen.getByText(/return retry/)).toBeTruthy();
    expect(screen.getByText(/return post/)).toBeTruthy();
  });

  it("marks the add and remove rows by tag", () => {
    const step = session.steps[3];
    const { container } = render(<DiffView hunks={step.hunks} />);
    expect(container.querySelectorAll("tr.add").length).toBeGreaterThan(0);
    expect(container.querySelectorAll("tr.remove").length).toBeGreaterThan(0);
  });

  it("renders a message, not a crash, without hunks (Review Focus 5)", () => {
    const { container } = render(<DiffView hunks={undefined} />);
    expect(container.textContent).toContain("No diff");
  });

  it("renders a message for an empty hunk list", () => {
    const { container } = render(<DiffView hunks={[]} />);
    expect(container.textContent).toContain("No diff");
  });

  it("truncates a long diff and offers the full one", () => {
    const lines = Array.from({ length: 80 }, (_, i) => ({
      tag: "add" as const,
      text: `line ${i}`,
    }));
    const hunks = [{ old_start: 1, new_start: 1, lines }];

    const { container } = render(<DiffView hunks={hunks} />);

    // 60 rows plus the truncation marker.
    expect(container.querySelectorAll("tr.add").length).toBe(60);
    expect(container.querySelector("tr.diff-truncated")).not.toBeNull();
    expect(screen.getByText(/show full diff/i)).toBeTruthy();
  });

  it("shows every line once expanded", () => {
    const lines = Array.from({ length: 80 }, (_, i) => ({
      tag: "add" as const,
      text: `line ${i}`,
    }));
    const hunks = [{ old_start: 1, new_start: 1, lines }];

    const { container } = render(<DiffView hunks={hunks} />);
    fireEvent.click(screen.getByText(/show full diff/i));

    expect(container.querySelectorAll("tr.add").length).toBe(80);
    expect(container.querySelector("tr.diff-truncated")).toBeNull();
  });
});
