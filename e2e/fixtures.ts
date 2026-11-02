/** Shared helpers: open the workbench with the injected sample project and fail on page errors. */
import { expect, test as base, type Page } from "@playwright/test";

export const PROJECT = "acme-api";

export const test = base.extend<{ workbench: Page }>({
  workbench: async ({ page }, provide) => {
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    // CodeMirror picks Mod-key bindings from navigator.platform, not the user agent.
    await page.addInitScript(() => Object.defineProperty(Navigator.prototype, "platform", { get: () => "MacIntel" }));
    await page.goto("/");
    await expect(page.getByRole("tree", { name: "Project" })).toBeVisible();
    await expect(page.getByRole("treeitem", { name: /acme-api/ })).toBeVisible();
    await provide(page);
    expect(errors, "uncaught page errors").toEqual([]);
  },
});

export { expect };

/** Tool windows currently laid out (visible) in the workbench. */
export async function visibleToolWindows(page: Page): Promise<string[]> {
  return page
    .locator("section[aria-label]")
    .evaluateAll((els) =>
      els
        .filter((e) => (e as HTMLElement).offsetParent !== null && e.getBoundingClientRect().width > 0)
        .map((e) => e.getAttribute("aria-label") ?? ""),
    );
}

export async function doubleShift(page: Page) {
  await page.keyboard.press("Shift");
  await page.keyboard.press("Shift");
}

/** Opens a sample file from the Project tree (a path may name its folders) and waits for the editor. */
export async function openFromTree(page: Page, path: string) {
  const parts = path.split("/");
  for (const dir of parts.slice(0, -1)) {
    const item = page.getByRole("treeitem", { name: dir, exact: true });
    if ((await item.getAttribute("aria-expanded")) !== "true") await item.dblclick();
  }
  const name = parts.at(-1)!;
  await page.getByRole("treeitem", { name, exact: true }).dblclick();
  await expect(page.getByRole("tablist", { name: "Editor tabs" }).getByRole("tab", { name })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const editor = page.locator(".cm-editor").first();
  await expect(editor.locator(".cm-content")).not.toBeEmpty();
  return editor;
}
