import { expect, test, type Locator, type Page } from "@playwright/test";

const VIEWPORT = { width: 1600, height: 1000 };

type Box = { x: number; y: number; width: number; height: number };

async function boxOf(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox();
  expect(box, await locator.evaluate((el) => el.className || (el as HTMLElement).id)).not.toBeNull();
  return box as Box;
}

function expectFillsViewport(name: string, box: Box): void {
  expect(Math.abs(box.x), `${name} x ${box.x}`).toBeLessThanOrEqual(2);
  expect(Math.abs(box.y), `${name} y ${box.y}`).toBeLessThanOrEqual(2);
  expect(Math.abs(box.width - VIEWPORT.width), `${name} width ${box.width}`).toBeLessThanOrEqual(2);
  expect(Math.abs(box.height - VIEWPORT.height), `${name} height ${box.height}`).toBeLessThanOrEqual(2);
}

function expectInsideViewport(name: string, box: Box): void {
  expect(box.x, `${name} left`).toBeGreaterThanOrEqual(-1);
  expect(box.y, `${name} top`).toBeGreaterThanOrEqual(-1);
  expect(box.x + box.width, `${name} right ${box.x + box.width}`).toBeLessThanOrEqual(VIEWPORT.width + 1);
  expect(box.y + box.height, `${name} bottom ${box.y + box.height}`).toBeLessThanOrEqual(VIEWPORT.height + 1);
  expect(box.width, `${name} has width`).toBeGreaterThan(0);
  expect(box.height, `${name} has height`).toBeGreaterThan(0);
}

async function setTextSize(page: Page, percent: string): Promise<void> {
  await page.getByRole("button", { name: "Settings" }).click();
  const slider = page.locator("#settings-text-size");
  await expect(slider).toBeVisible();
  await slider.evaluate((el, next) => {
    const input = el as HTMLInputElement;
    const proto = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
    proto?.set?.call(input, next);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }, percent);
  await expect(page.locator(".app-shell")).toHaveAttribute("style", new RegExp(`--ui-scale:\\s*${Number(percent) / 100}\\b`));
  await page.getByRole("dialog", { name: "Settings" }).getByRole("button", { name: "Close" }).click();
  await expect(page.getByRole("dialog", { name: "Settings" })).toBeHidden();
}

test("at 200% the shell fills a 1600 by 1000 window and the terminal and command box stay inside it", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("commandui.welcome.showAtStartup", "false");
  });
  await page.setViewportSize(VIEWPORT);
  await page.goto("/");

  const shell = page.locator(".app-shell");
  const terminal = page.locator(".terminal-shell");
  const command = page.locator("#command-box");
  await expect(shell).toBeVisible();
  await expect(terminal).toBeVisible();
  await expect(command).toBeVisible();

  await setTextSize(page, "200");

  expectFillsViewport("shell", await boxOf(shell));
  expectInsideViewport("terminal", await boxOf(terminal));
  expectInsideViewport("command box", await boxOf(command));

  const openHeight = (await boxOf(terminal)).height;
  await page.getByRole("button", { name: "Hide activity" }).click();
  await expect(page.getByRole("log", { name: "CommandUI activity" })).toBeHidden();
  await expect.poll(async () => (await terminal.boundingBox())?.height ?? 0).toBeGreaterThan(openHeight);
  expectInsideViewport("terminal after hide", await boxOf(terminal));
  expectInsideViewport("command box after hide", await boxOf(command));
});
