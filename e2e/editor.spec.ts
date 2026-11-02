import { expect, openFromTree, test } from "./fixtures";

test("opens a file from the project tree and types into it", async ({ workbench: page }) => {
  const editor = await openFromTree(page, "CMakeLists.txt");
  await editor.locator(".cm-content").click();
  await page.keyboard.press("Meta+ArrowDown");
  await page.keyboard.press("Enter");
  await page.keyboard.type("# typed by e2e");
  await expect(editor.locator(".cm-content")).toContainText("# typed by e2e");
  // Undo removes the typing again.
  await page.keyboard.press("Meta+z");
  await expect(editor.locator(".cm-content")).not.toContainText("# typed by e2e");
});

test("⌘F opens the find bar and highlights matches; Escape returns to the editor", async ({ workbench: page }) => {
  const editor = await openFromTree(page, "CMakeLists.txt");
  await editor.locator(".cm-content").click();
  await page.keyboard.press("Meta+f");
  const find = page.getByRole("textbox", { name: "Find" });
  await expect(find).toBeFocused();
  await find.fill("acme");
  await expect(editor.locator(".cm-openclion-findMatch").first()).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(find).toBeHidden();
  await expect(editor.locator(".cm-content")).toBeFocused();
});

test("opening several files creates tabs and ⌘W closes the active one", async ({ workbench: page }) => {
  await openFromTree(page, "CMakeLists.txt");
  await openFromTree(page, "README.md");
  const tabs = page.getByRole("tablist", { name: "Editor tabs" }).getByRole("tab");
  await expect(tabs).toHaveCount(2);
  await page.locator(".cm-editor .cm-content").first().click();
  await page.keyboard.press("Meta+w");
  await expect(tabs).toHaveCount(1);
  await expect(tabs.first()).toHaveText(/CMakeLists\.txt/);
});

test("breadcrumbs follow the caret into the symbol under it", async ({ workbench: page }) => {
  const editor = await openFromTree(page, "include/greeter.h");
  const status = page.getByRole("navigation", { name: "Breadcrumbs", exact: true });
  const code = page.getByRole("navigation", { name: "Code breadcrumbs" });
  await editor.locator(".cm-line", { hasText: "std::string greet() const;" }).click();
  await expect(status).toContainText("greeter.h");
  await expect(code).toContainText("Greeter");
  // Typing inside the class keeps its crumb.
  await page.keyboard.press("Home");
  await page.keyboard.type("// ");
  await expect(code).toContainText("Greeter");
  // ⌘↑ opens the navigation bar at the caret's symbol.
  await page.keyboard.press("Meta+ArrowUp");
  const bar = page.getByRole("toolbar", { name: "Navigation bar" });
  await expect(bar).toContainText("Greeter");
  await page.keyboard.press("Escape");
  await expect(bar).toBeHidden();
});
