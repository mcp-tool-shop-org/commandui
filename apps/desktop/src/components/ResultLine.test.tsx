import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describeResult } from "../lib/commandResult";
import { ResultLine } from "./ResultLine";

describe("ResultLine", () => {
  it("renders each failure cause in words, with its actions, and never the word FAILURE", () => {
    const cases = [
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: "nope" }),
      describeResult({ phase: "failure", exitCode: 127, exitKnown: true }),
      describeResult({ phase: "failure", exitCode: 5, exitKnown: true }),
      describeResult({
        phase: "failure",
        exitCode: 1,
        exitKnown: true,
        outputText: "The system cannot find the file specified.",
      }),
      describeResult({ phase: "unknown", exitCode: 1, exitKnown: false, reason: "exit_unknown" }),
      describeResult({ phase: "failure", exitCode: 1, exitKnown: false, reason: "shell_exited" }),
      describeResult({ phase: "failure", exitCode: 1, exitKnown: false, reason: "input_not_accepted" }),
      describeResult({ phase: "request", command: "list the files" }),
      describeResult({ phase: "running" }),
      describeResult({ phase: "success", outputLines: 24 }),
    ];

    for (const result of cases) {
      const { unmount } = render(<ResultLine result={result} />);
      const line = screen.getByTestId("result-line");
      expect(line.textContent ?? "").not.toMatch(/\bFAILURE\b/);
      expect((line.textContent ?? "").trim().toLowerCase()).not.toBe("failure");
      expect(line).toHaveTextContent(result.headline);
      unmount();
    }
  });

  it("offers Show output, Ask how to fix it, and Run again for a failure", async () => {
    const onAction = vi.fn();
    const user = userEvent.setup();
    render(
      <ResultLine
        result={describeResult({ phase: "failure", exitCode: 1, exitKnown: true })}
        onAction={onAction}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Show output" }));
    await user.click(screen.getByRole("button", { name: "Ask how to fix it" }));
    await user.click(screen.getByRole("button", { name: "Run again" }));
    expect(onAction.mock.calls.map((call) => call[0])).toEqual(["show-output", "ask-fix", "run-again"]);
  });

  it("offers Ask for a sentence and Stop while a command runs", () => {
    const { rerender } = render(
      <ResultLine result={describeResult({ phase: "request" })} onAction={() => {}} />,
    );
    expect(screen.getByRole("button", { name: "Ask CommandUI instead" })).toBeInTheDocument();
    rerender(<ResultLine result={describeResult({ phase: "running" })} onAction={() => {}} />);
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
    expect(screen.queryByText("failure")).not.toBeInTheDocument();
  });
});
