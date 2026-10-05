import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkflowEditor } from "./WorkflowEditor";

function renderEditor(
  steps = ["git status", "git diff"],
  label = "Ship",
) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  render(
    <WorkflowEditor
      initialLabel={label}
      initialSteps={steps}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  );
  return { onConfirm, onCancel };
}

describe("WorkflowEditor", () => {
  it("confirms the trimmed name and steps once", async () => {
    const user = userEvent.setup();
    const { onConfirm } = renderEditor();
    const name = screen.getByDisplayValue("Ship");
    await user.clear(name);
    await user.type(name, "  Ship it  ");
    const confirm = screen.getByRole("button", { name: "Create workflow" });
    await user.click(confirm);
    await user.click(confirm);
    expect(onConfirm).toHaveBeenCalledOnce();
    expect(onConfirm).toHaveBeenCalledWith("Ship it", ["git status", "git diff"]);
  });

  it("refuses a blank name or a blank step", async () => {
    const user = userEvent.setup();
    const { onConfirm } = renderEditor(["git status"], "Ship");
    const confirm = screen.getByRole("button", { name: "Create workflow" });
    await user.clear(screen.getByDisplayValue("Ship"));
    expect(confirm).toBeDisabled();

    await user.type(screen.getAllByRole("textbox")[0], "Named");
    await user.clear(screen.getByDisplayValue("git status"));
    expect(confirm).toBeDisabled();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("reorders steps and will not remove the last one", async () => {
    const user = userEvent.setup();
    const { onConfirm } = renderEditor(["first", "second"]);
    expect(screen.getByRole("button", { name: "Move step 1 up" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move step 2 down" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Move step 1 down" }));
    await user.click(screen.getByRole("button", { name: "Remove step 1" }));
    expect(screen.getByRole("button", { name: "Remove step 1" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Create workflow" }));
    expect(onConfirm).toHaveBeenCalledWith("Ship", ["first"]);
  });

  it("cancels from the button, Escape, and the backdrop, not from the panel", async () => {
    const user = userEvent.setup();
    const { onCancel } = renderEditor(["only"]);
    expect(screen.getByRole("button", { name: "Remove step 1" })).toBeDisabled();

    await user.click(screen.getByRole("heading", { name: "New workflow" }));
    expect(onCancel).not.toHaveBeenCalled();

    await user.click(screen.getByDisplayValue("only"));
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledOnce();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(2);

    await user.click(document.querySelector(".palette-overlay")!);
    expect(onCancel).toHaveBeenCalledTimes(3);
  });
});
