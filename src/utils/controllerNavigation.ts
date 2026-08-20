export type Direction = "up" | "down" | "left" | "right";

const CONTROLLER_FOCUS_ATTR = "data-controller-focus";
const CONFIRM_BUTTON = 0;
const CANCEL_BUTTONS = [1, 8];
const FOCUSABLE_SELECTOR = [
  "button",
  "input",
  "select",
  "textarea",
  "a[href]",
  "[tabindex]:not([tabindex='-1'])",
  "[role='button']",
].join(",");

interface ControllerButtonHandlers {
  onCancel: () => void;
  onConfirm?: () => void;
}

interface ControllerNavigationOptions {
  root?: ParentNode | (() => ParentNode);
  onCancel: () => void;
  axisThreshold?: number;
  repeatDelayMs?: number;
  repeatIntervalMs?: number;
}

function resolveRoot(root?: ParentNode | (() => ParentNode)): ParentNode {
  return typeof root === "function" ? root() : (root ?? document);
}

function rootContains(root: ParentNode, element: HTMLElement): boolean {
  if (root instanceof Document) return root.body.contains(element);
  return root.contains(element);
}

function isVisible(element: HTMLElement): boolean {
  if (element.hidden) return false;
  if (
    element.closest("[hidden], [aria-hidden='true']") ||
    element.closest("[inert]")
  ) {
    return false;
  }
  const style = window.getComputedStyle(element);
  if (style.display === "none" || style.visibility === "hidden") {
    return false;
  }
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}

function isEnabled(element: HTMLElement): boolean {
  if (element.getAttribute("aria-disabled") === "true") return false;
  if ("disabled" in element && Boolean(element.disabled)) return false;
  return true;
}

function isFocusable(element: HTMLElement): boolean {
  return isEnabled(element) && isVisible(element);
}

function center(element: HTMLElement) {
  const rect = element.getBoundingClientRect();
  return {
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
  };
}

export function getFocusableElements(root: ParentNode = document) {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => rootContains(root, element))
    .filter(isFocusable);
}

export function clearControllerFocus(root: ParentNode = document) {
  root
    .querySelectorAll<HTMLElement>(`[${CONTROLLER_FOCUS_ATTR}]`)
    .forEach((element) => element.removeAttribute(CONTROLLER_FOCUS_ATTR));
}

export function getControllerFocusedElement(
  root: ParentNode = document,
): HTMLElement | null {
  const element = root.querySelector<HTMLElement>(
    `[${CONTROLLER_FOCUS_ATTR}='true']`,
  );
  return element && isFocusable(element) ? element : null;
}

export function findNearestFocusTarget(
  current: HTMLElement | null,
  direction: Direction,
  candidates: HTMLElement[],
): HTMLElement | null {
  if (candidates.length === 0) return null;
  if (!current || !candidates.includes(current)) return candidates[0];

  const from = center(current);
  let best: { element: HTMLElement; score: number } | null = null;

  for (const candidate of candidates) {
    if (candidate === current) continue;
    const to = center(candidate);
    const dx = to.x - from.x;
    const dy = to.y - from.y;
    const primary =
      direction === "right"
        ? dx
        : direction === "left"
          ? -dx
          : direction === "down"
            ? dy
            : -dy;
    if (primary <= 0) continue;

    const cross = direction === "left" || direction === "right" ? dy : dx;
    const score = primary + Math.abs(cross) * 1.8;
    if (!best || score < best.score) {
      best = { element: candidate, score };
    }
  }

  return best?.element ?? null;
}

export function focusControllerElement(
  element: HTMLElement | null,
  root: ParentNode = document,
): HTMLElement | null {
  if (!element || !isFocusable(element)) return null;
  clearControllerFocus(root);
  element.setAttribute(CONTROLLER_FOCUS_ATTR, "true");
  element.focus({ preventScroll: true });
  element.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  return element;
}

export function moveControllerFocus(
  direction: Direction,
  root: ParentNode = document,
): HTMLElement | null {
  const candidates = getFocusableElements(root);
  const active =
    document.activeElement instanceof HTMLElement &&
    rootContains(root, document.activeElement) &&
    isFocusable(document.activeElement)
      ? document.activeElement
      : null;
  const current = getControllerFocusedElement(root) ?? active;
  return focusControllerElement(
    findNearestFocusTarget(current, direction, candidates),
    root,
  );
}

export function activateFocusedElement(root: ParentNode = document): boolean {
  const active =
    document.activeElement instanceof HTMLElement &&
    rootContains(root, document.activeElement) &&
    isFocusable(document.activeElement)
      ? document.activeElement
      : null;
  const target = getControllerFocusedElement(root) ?? active;
  if (!target) return false;
  target.click();
  return true;
}

export function handleControllerButtons(
  previousButtons: Set<number>,
  pressedButtons: Set<number>,
  handlers: ControllerButtonHandlers,
): boolean {
  const pressedCancel = CANCEL_BUTTONS.some(
    (button) => pressedButtons.has(button) && !previousButtons.has(button),
  );
  if (pressedCancel) {
    handlers.onCancel();
    return true;
  }

  if (pressedButtons.has(CONFIRM_BUTTON) && !previousButtons.has(CONFIRM_BUTTON)) {
    handlers.onConfirm?.();
    return true;
  }

  return false;
}

export function getPressedGamepadButtons(gamepads: Gamepad[]): Set<number> {
  const pressed = new Set<number>();
  for (const gamepad of gamepads) {
    gamepad.buttons.forEach((button, index) => {
      if (button.pressed) pressed.add(index);
    });
  }
  return pressed;
}

export function getGamepadDirection(
  gamepads: Gamepad[],
  threshold = 0.55,
): Direction | null {
  for (const gamepad of gamepads) {
    if (gamepad.buttons[12]?.pressed) return "up";
    if (gamepad.buttons[13]?.pressed) return "down";
    if (gamepad.buttons[14]?.pressed) return "left";
    if (gamepad.buttons[15]?.pressed) return "right";

    const x = gamepad.axes[0] ?? 0;
    const y = gamepad.axes[1] ?? 0;
    if (Math.abs(x) > Math.abs(y) && Math.abs(x) >= threshold) {
      return x > 0 ? "right" : "left";
    }
    if (Math.abs(y) >= threshold) {
      return y > 0 ? "down" : "up";
    }
  }
  return null;
}

export function startControllerNavigation(
  options: ControllerNavigationOptions,
): () => void {
  if (typeof window === "undefined" || typeof navigator === "undefined") {
    return () => {};
  }
  if (typeof navigator.getGamepads !== "function") {
    return () => {};
  }

  let frame = 0;
  let stopped = false;
  let previousButtons = new Set<number>();
  let heldDirection: Direction | null = null;
  let nextMoveAt = 0;

  const repeatDelayMs = options.repeatDelayMs ?? 320;
  const repeatIntervalMs = options.repeatIntervalMs ?? 150;
  const axisThreshold = options.axisThreshold ?? 0.55;

  const tick = (now: number) => {
    if (stopped) return;

    const root = resolveRoot(options.root);
    const gamepads = Array.from(navigator.getGamepads()).filter(
      Boolean,
    ) as Gamepad[];
    const pressedButtons = getPressedGamepadButtons(gamepads);

    handleControllerButtons(previousButtons, pressedButtons, {
      onCancel: options.onCancel,
      onConfirm: () => activateFocusedElement(root),
    });
    previousButtons = pressedButtons;

    const direction = getGamepadDirection(gamepads, axisThreshold);
    if (direction) {
      if (direction !== heldDirection || now >= nextMoveAt) {
        moveControllerFocus(direction, root);
        nextMoveAt =
          now + (direction === heldDirection ? repeatIntervalMs : repeatDelayMs);
        heldDirection = direction;
      }
    } else {
      heldDirection = null;
      nextMoveAt = 0;
    }

    frame = window.requestAnimationFrame(tick);
  };

  frame = window.requestAnimationFrame(tick);

  return () => {
    stopped = true;
    window.cancelAnimationFrame(frame);
    clearControllerFocus(resolveRoot(options.root));
  };
}
