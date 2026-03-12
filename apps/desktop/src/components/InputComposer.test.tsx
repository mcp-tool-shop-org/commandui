import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { InputComposer } from "./InputComposer";

describe("InputComposer", () => {
  it("submits on Enter", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={() => {}}
        onSubmit={onSubmit}
      />,
    );

    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "git status{Enter}");

    expect(onSubmit).toHaveBeenCalledWith("git status");
  });

  it("switches mode on button click", async () => {
    const onModeChange = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={onModeChange}
        onSubmit={() => {}}
      />,
    );

    await userEvent.click(screen.getByText("Ask"));
    expect(onModeChange).toHaveBeenCalledWith("ask");
  });

  it("does not submit empty input", async () => {
    const onSubmit = vi.fn();
    render(
      <InputComposer
        mode="command"
        onModeChange={() => {}}
        onSubmit={onSubmit}
      />,
    );

    const input = screen.getByPlaceholderText(/submit a command/i);
    await userEvent.type(input, "{Enter}");

    expect(onSubmit).not.toHaveBeenCalled();
  });
});
