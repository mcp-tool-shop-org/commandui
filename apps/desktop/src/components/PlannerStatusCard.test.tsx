import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { PlannerStatus } from "@commandui/api-contract";
import { PlannerStatusCard } from "./PlannerStatusCard";

const STATES: PlannerStatus[] = [
  {
    state: "notInstalled",
    model: "qwen2.5:14b",
    endpoint: "http://localhost:11434",
    headline: "Ollama is not installed.",
    fix: "Install Ollama, then download qwen2.5:14b.",
    link: "https://ollama.com/download",
    linkLabel: "Download Ollama",
  },
  {
    state: "notRunning",
    model: "qwen2.5:14b",
    endpoint: "http://localhost:11434",
    headline: "Ollama is not running.",
    fix: "Start Ollama, then choose Check again.",
    link: "https://ollama.com/download",
    linkLabel: "Download Ollama",
  },
  {
    state: "modelMissing",
    model: "qwen2.5:14b",
    endpoint: "http://localhost:11434",
    headline: "The model qwen2.5:14b is not downloaded.",
    fix: "Download it with: ollama pull qwen2.5:14b",
    link: "https://ollama.com/library/qwen2.5",
    linkLabel: "Model page",
  },
  {
    state: "ready",
    model: "qwen2.5:14b",
    endpoint: "http://localhost:11434",
    headline: "Ready.",
    fix: "Ask can draft a command with qwen2.5:14b.",
    link: "https://ollama.com/library",
    linkLabel: "Model library",
  },
];

describe("PlannerStatusCard", () => {
  it.each(STATES)("shows $state with its fix and link", async (status) => {
    const onCheckAgain = vi.fn();
    render(<PlannerStatusCard status={status} onCheckAgain={onCheckAgain} />);
    expect(screen.getByText(status.headline)).toBeInTheDocument();
    expect(screen.getByText(status.fix)).toBeInTheDocument();
    const link = screen.getByRole("link", { name: status.linkLabel });
    expect(link).toHaveAttribute("href", status.link);
    expect(screen.queryByRole("button", { name: "Run Plan" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(onCheckAgain).toHaveBeenCalledTimes(1);
  });
});
