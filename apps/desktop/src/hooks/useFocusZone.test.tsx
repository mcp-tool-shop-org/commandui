import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { useFocusStore } from "@commandui/state";
import { useFocusZone } from "./useFocusZone";

function Zone({ active }: { active: boolean }) {
  useFocusZone("plan", active);
  return null;
}

describe("useFocusZone", () => {
  it("claims the zone only while the surface is active", () => {
    useFocusStore.setState({ currentZone: null, previousZone: null });
    const view = render(<Zone active={false} />);
    expect(useFocusStore.getState().currentZone).toBeNull();

    view.rerender(<Zone active />);
    expect(useFocusStore.getState().currentZone).toBe("plan");
    expect(useFocusStore.getState().previousZone).toBeNull();
  });
});
