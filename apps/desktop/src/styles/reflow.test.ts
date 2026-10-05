import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "globals.css"), "utf8");

function blockAfter(source: string, start: string, end: string): string {
  const at = source.indexOf(start);
  expect(at).toBeGreaterThanOrEqual(0);
  const close = source.indexOf(end, at);
  expect(close).toBeGreaterThan(at);
  return source.slice(at, close);
}

describe("narrow layout", () => {
  it("reflows from the shell's layout width, not the window", () => {
    expect(css).toContain("container-name: commandui");
    expect(css).toContain("container-type: inline-size");
    const query = blockAfter(css, "@container commandui (max-width: 960px)", ".app-shell.layout-narrow .main-layout");
    expect(query).toContain("grid-template-columns: minmax(0, 1fr)");
    expect(query).toContain("min-height: 12rem");
    expect(query).toContain("border-top: 1px solid var(--border)");
    expect(query).not.toContain("340px");

    const measured = blockAfter(css, ".app-shell.layout-narrow .main-layout", ":focus-visible");
    expect(measured).toContain("grid-template-columns: minmax(0, 1fr)");
    expect(measured).toContain("min-height: 12rem");
    expect(measured).toContain("min-height: 22rem");
  });

  it("keeps the header path on one line and gives every focus a ring", () => {
    const title = blockAfter(css, ".topbar-title {", ".topbar-actions");
    expect(title).toContain("white-space: nowrap");
    expect(title).toContain("text-overflow: ellipsis");
    expect(title).not.toContain("break-all");

    expect(css).toContain(":focus-visible");
    expect(css).toContain(".terminal-shell:focus-within");
    const badge = blockAfter(css, ".memory-kind-badge {", ".memory-label");
    expect(badge).not.toContain("uppercase");
    const memory = blockAfter(css, ".memory-panel {", ".memory-item");
    expect(memory).toContain("max-height: 8rem");
    expect(memory).not.toContain("30vh");

    const drawer = blockAfter(css, ".history-drawer {", ".history-controls");
    expect(drawer).toContain("overflow-x: hidden");
    const historyTitle = blockAfter(css, ".history-main {", ".history-meta");
    expect(historyTitle).toContain("min-width: 0");
    expect(historyTitle).toContain("text-overflow: ellipsis");
    expect(historyTitle).toContain("white-space: nowrap");
  });
});
