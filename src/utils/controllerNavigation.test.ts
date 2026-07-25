import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  activateFocusedElement,
  findNearestFocusTarget,
  getFocusableElements,
  handleControllerButtons,
} from "./controllerNavigation";

function rect(
  element: HTMLElement,
  left: number,
  top: number,
  width = 40,
  height = 30,
) {
  element.getBoundingClientRect = vi.fn(
    () =>
      ({
        left,
        top,
        right: left + width,
        bottom: top + height,
        width,
        height,
        x: left,
        y: top,
        toJSON: () => {},
      }) as DOMRect,
  );
}

describe("controllerNavigation", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("filters disabled and hidden focusable elements", () => {
    const enabled = document.createElement("button");
    const disabled = document.createElement("button");
    const hidden = document.createElement("button");

    disabled.disabled = true;
    hidden.hidden = true;

    document.body.append(enabled, disabled, hidden);
    rect(enabled, 0, 0);
    rect(disabled, 50, 0);
    rect(hidden, 100, 0);

    expect(getFocusableElements()).toEqual([enabled]);
  });

  it("finds the nearest focus target in the requested direction", () => {
    const current = document.createElement("button");
    const rightNear = document.createElement("button");
    const rightFar = document.createElement("button");
    const below = document.createElement("button");

    document.body.append(current, rightFar, below, rightNear);
    rect(current, 0, 0);
    rect(rightNear, 70, 5);
    rect(rightFar, 160, 0);
    rect(below, 0, 80);

    expect(
      findNearestFocusTarget(current, "right", [
        current,
        rightFar,
        below,
        rightNear,
      ]),
    ).toBe(rightNear);
  });

  it("activates the controller-focused element", () => {
    const button = document.createElement("button");
    const onClick = vi.fn();

    button.addEventListener("click", onClick);
    button.dataset.controllerFocus = "true";
    document.body.append(button);
    rect(button, 0, 0);

    expect(activateFocusedElement()).toBe(true);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it("calls cancel for B and Back button presses", () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();

    handleControllerButtons(new Set(), new Set([1]), {
      onCancel,
      onConfirm,
    });
    handleControllerButtons(new Set(), new Set([8]), {
      onCancel,
      onConfirm,
    });

    expect(onCancel).toHaveBeenCalledTimes(2);
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
