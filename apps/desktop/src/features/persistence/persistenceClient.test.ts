import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../lib/tauriInvoke", () => ({
  tauriInvoke: vi.fn(),
}));

import type { Workflow } from "@commandui/domain";
import { tauriInvoke } from "../../lib/tauriInvoke";
import { workflowAdd, workflowList } from "./persistenceClient";

const invoke = vi.mocked(tauriInvoke);

const workflow: Workflow = {
  id: "w1",
  label: "ship it",
  source: "promoted",
  command: "git add . && git status",
  steps: [{ command: "git add ." }, { command: "git status", label: "look" }],
  createdAt: "2026-10-04T00:00:00Z",
};

describe("workflow persistence wire", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("sends stepsJson, which is the column Rust stores", async () => {
    invoke.mockResolvedValue({ ok: true });
    await workflowAdd({ workflow });
    expect(invoke).toHaveBeenCalledWith("workflow_add", {
      request: {
        workflow: {
          ...workflow,
          stepsJson: JSON.stringify(workflow.steps),
        },
      },
    });
  });

  it("restores steps from a list row that has only stepsJson", async () => {
    invoke.mockResolvedValue({
      workflows: [
        {
          id: workflow.id,
          label: workflow.label,
          source: workflow.source,
          command: workflow.command,
          stepsJson: JSON.stringify(workflow.steps),
          createdAt: workflow.createdAt,
        },
      ],
    });
    const listed = await workflowList();
    expect(listed.workflows).toEqual([workflow]);
  });
});
