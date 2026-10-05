import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsDrawer } from "./SettingsDrawer";

function renderDrawer(overrides: Partial<Parameters<typeof SettingsDrawer>[0]> = {}) {
  const props = {
    isOpen: true,
    onClose: vi.fn(),
    productMode: "guided" as const,
    onProductModeChange: vi.fn(),
    defaultInputMode: "ask" as const,
    onDefaultInputModeChange: vi.fn(),
    fontSize: "md",
    onFontSizeChange: vi.fn(),
    simplifiedSummaries: false,
    onSimplifiedSummariesChange: vi.fn(),
    plannerModel: "qwen2.5:14b",
    onPlannerModelChange: vi.fn(),
    plannerEndpoint: "http://localhost:11434",
    onPlannerEndpointChange: vi.fn(),
    plannerStatus: null,
    onCheckPlanner: vi.fn(),
    ...overrides,
  };
  render(<SettingsDrawer {...props} />);
  return props;
}

describe("SettingsDrawer", () => {
  it("renders nothing while it is closed", () => {
    renderDrawer({ isOpen: false });
    expect(screen.queryByText("Settings")).toBeNull();
  });

  it("reports each setting the user changes and closes from the button or the backdrop", async () => {
    const user = userEvent.setup();
    const props = renderDrawer();

    const [mode, inputMode] = screen.getAllByRole("combobox");
    await user.selectOptions(mode, "classic");
    expect(props.onProductModeChange).toHaveBeenCalledWith("classic");

    await user.selectOptions(inputMode, "command");
    expect(props.onDefaultInputModeChange).toHaveBeenCalledWith("command");

    await user.click(screen.getByLabelText("Simplified summaries"));
    expect(props.onSimplifiedSummariesChange).toHaveBeenCalledWith(true);

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(props.onClose).toHaveBeenCalledOnce();

    await user.click(screen.getByText("Settings"));
    expect(props.onClose).toHaveBeenCalledOnce();
    await user.click(document.querySelector(".settings-overlay")!);
    expect(props.onClose).toHaveBeenCalledTimes(2);
  });
});
