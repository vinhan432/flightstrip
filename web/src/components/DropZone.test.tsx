import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { DropZone } from "./DropZone";

function fileWith(content: string, name = "x.json"): File {
  return new File([content], name, { type: "application/json" });
}

describe("DropZone", () => {
  it("loads a valid session and reports it", async () => {
    const onLoad = vi.fn();
    render(<DropZone onLoad={onLoad} />);

    const zone = screen.getByRole("application", { name: /drop/i });
    const valid = JSON.stringify({
      version: 1,
      meta: {
        id: "x",
        title: "Test session",
        started_at: "2026-01-01T00:00:00Z",
        duration_ms: 1000,
        os: "windows",
        repo_name: "repo",
        label: "fixture",
      },
      prompts: [],
      steps: [],
      incidents: [],
      files: {},
      stats: {},
    });

    fireEvent.drop(zone, { dataTransfer: { files: [fileWith(valid)] } });

    await waitFor(() => expect(onLoad).toHaveBeenCalledTimes(1));
    expect(onLoad.mock.calls[0][0].meta.title).toBe("Test session");
  });

  it("reports an invalid file instead of crashing (Review Focus 4)", async () => {
    const onLoad = vi.fn();
    render(<DropZone onLoad={onLoad} />);

    const zone = screen.getByRole("application", { name: /drop/i });
    fireEvent.drop(zone, { dataTransfer: { files: [fileWith("not json")] } });

    expect(await screen.findByText(/invalid session file/i)).toBeTruthy();
    expect(onLoad).not.toHaveBeenCalled();
  });

  it("names the bad field when the shape is wrong", async () => {
    render(<DropZone onLoad={() => {}} />);
    const zone = screen.getByRole("application", { name: /drop/i });
    fireEvent.drop(zone, {
      dataTransfer: { files: [fileWith(JSON.stringify({ version: 1 }))] },
    });

    expect(await screen.findByText(/invalid session file/i)).toBeTruthy();
    expect(screen.getByText(/meta/i)).toBeTruthy();
  });

  it("shows the release hint while dragging over", () => {
    render(<DropZone onLoad={() => {}} />);
    const zone = screen.getByRole("application", { name: /drop/i });
    fireEvent.dragOver(zone, { dataTransfer: { files: [] } });
    expect(screen.getByText(/release to load/i)).toBeTruthy();
  });

  it("clears the error when a valid file is dropped next", async () => {
    const onLoad = vi.fn();
    render(<DropZone onLoad={onLoad} />);
    const zone = screen.getByRole("application", { name: /drop/i });

    fireEvent.drop(zone, { dataTransfer: { files: [fileWith("nope")] } });
    expect(await screen.findByText(/invalid session file/i)).toBeTruthy();

    const valid = JSON.stringify({
      version: 1,
      meta: {
        id: "x",
        title: "Second try",
        started_at: "2026-01-01T00:00:00Z",
        duration_ms: 1,
        os: "linux",
        repo_name: "repo",
        label: "fixture",
      },
      prompts: [],
      steps: [],
      incidents: [],
      files: {},
      stats: {},
    });
    fireEvent.drop(zone, { dataTransfer: { files: [fileWith(valid)] } });

    await waitFor(() => expect(onLoad).toHaveBeenCalled());
    expect(screen.queryByText(/invalid session file/i)).toBeNull();
  });
});
