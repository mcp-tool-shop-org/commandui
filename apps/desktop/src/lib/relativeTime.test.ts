import { describe, expect, it } from "vitest";
import { formatRelativeTime } from "./relativeTime";

const NOW = Date.parse("2026-10-04T12:00:00.000Z");

describe("formatRelativeTime", () => {
  it("says just now inside a minute", () => {
    expect(formatRelativeTime(NOW - 30_000, NOW)).toBe("just now");
  });

  it("spells out one and many minutes, hours, and days", () => {
    expect(formatRelativeTime(NOW - 60_000, NOW)).toBe("1 minute ago");
    expect(formatRelativeTime(NOW - 2 * 60_000, NOW)).toBe("2 minutes ago");
    expect(formatRelativeTime(NOW - 3_600_000, NOW)).toBe("1 hour ago");
    expect(formatRelativeTime(NOW - 5 * 3_600_000, NOW)).toBe("5 hours ago");
    expect(formatRelativeTime(NOW - 86_400_000, NOW)).toBe("1 day ago");
    expect(formatRelativeTime(NOW - 2 * 86_400_000, NOW)).toBe("2 days ago");
  });
});
