import { describe, expect, it } from "vitest";
import { hydrateWorkflow, toStoredWorkflow, type Workflow } from "./workflow";

function promoted(): Workflow {
  return {
    id: "w1",
    label: "ship it",
    source: "promoted",
    command: "git add . && git status",
    steps: [{ command: "git add ." }, { command: "git status", label: "look" }],
    projectRoot: "E:/work",
    createdAt: "2026-10-04T00:00:00Z",
  };
}

describe("workflow step persistence", () => {
  it("writes stepsJson and reads it back after serde drops steps", () => {
    const stored = toStoredWorkflow(promoted());
    expect(stored.stepsJson).toBe(
      JSON.stringify([{ command: "git add ." }, { command: "git status", label: "look" }]),
    );
    const listed = hydrateWorkflow({ ...stored, steps: undefined });
    expect(listed.steps).toEqual(promoted().steps);
    expect(listed).not.toHaveProperty("stepsJson");
  });

  it("stores null when the workflow has no step list", () => {
    const { steps: _steps, ...single } = promoted();
    const stored = toStoredWorkflow(single);
    expect(stored.stepsJson).toBeNull();
    expect(hydrateWorkflow(stored).steps).toBeUndefined();
  });

  it("accepts a legacy stepsJson array of command strings", () => {
    const hydrated = hydrateWorkflow({
      ...promoted(),
      steps: undefined,
      stepsJson: JSON.stringify(["git status", " git diff "]),
    });
    expect(hydrated.steps).toEqual([{ command: "git status" }, { command: "git diff" }]);
  });

  it("leaves steps unset when stepsJson is not a step list", () => {
    const broken = hydrateWorkflow({
      ...promoted(),
      steps: undefined,
      stepsJson: "{not json",
    });
    expect(broken.steps).toBeUndefined();
    const empty = hydrateWorkflow({
      ...promoted(),
      steps: undefined,
      stepsJson: "[]",
    });
    expect(empty.steps).toBeUndefined();
  });
});
