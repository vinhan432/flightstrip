import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
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
});
