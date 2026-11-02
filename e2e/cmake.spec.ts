/**
 * The CMake tool window in plain-browser mode: the sample project is a CMake
 * C++ tree, so the window registers itself like it does for a real project and
 * shows the model the backend would return.
 */
import type { Page } from "@playwright/test";
import { doubleShift, expect, openFromTree, test } from "./fixtures";

async function openCmakeWindow(page: Page) {
  const stripe = page.getByRole("toolbar", { name: "Right tool windows" });
  await stripe.getByRole("button", { name: "CMake", exact: true }).click();
  const window = page.getByRole("region", { name: "CMake" });
  await expect(window).toBeVisible();
  return window;
}

test("lists build directories, targets and the compile database", async ({ workbench: page }) => {
  const window = await openCmakeWindow(page);
  const tree = window.getByRole("tree", { name: "CMake" });
  await expect(tree.getByRole("treeitem", { name: /^acme-api$/ })).toBeVisible();
  await expect(tree.getByRole("treeitem", { name: /^Debug/ })).toBeVisible();
  await expect(tree.getByRole("treeitem", { name: /^Targets$/ })).toBeVisible();
  await expect(tree.getByRole("treeitem", { name: /^acme$/ })).toBeVisible();
  await expect(tree.getByRole("treeitem", { name: /^acme_tests$/ })).toBeVisible();
  // Non-executable targets carry their kind as IDEA-style detail text.
  await expect(tree.getByRole("treeitem", { name: /^greeter static library$/ })).toBeVisible();
  await expect(tree.getByRole("treeitem", { name: /^compile_commands\.json$/ })).toBeVisible();
});

test("keeps the workbench version and the CMake tool window", async ({ workbench: page }, info) => {
  await openFromTree(page, "src/main.cpp");
  await openCmakeWindow(page);
  await expect(page.locator(".cm-editor").first().locator(".cm-content")).toContainText("main");
  await page.screenshot({ path: "docs/media/openclion-cmake.png", animations: "disabled" });
  await info.attach("workbench", {
    body: await page.screenshot({ animations: "disabled" }),
    contentType: "image/png",
  });
});

test("Toolchains settings lists the detected C/C++ toolchain", async ({ workbench: page }) => {
  await page.keyboard.press("Meta+,");
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.getByRole("textbox", { name: "Search settings" }).fill("Toolchains");
  await dialog.getByRole("treeitem", { name: "Toolchains", exact: true }).click();
  const detected = dialog.getByRole("region", { name: "Detected toolchains" });
  await expect(detected.getByText("Clang 18.1.3")).toBeVisible();
  await expect(detected.getByText("/usr/bin/clang++")).toBeVisible();
  await expect(dialog.getByLabel("CMake")).toBeVisible();
  await expect(dialog.getByRole("checkbox", { name: "Report clang-tidy warnings in the editor" })).toBeChecked();
  await page.screenshot({ path: "docs/media/openclion-toolchains.png", animations: "disabled" });
});

test("CMake settings offer the preset mode", async ({ workbench: page }) => {
  await page.keyboard.press("Meta+,");
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.getByRole("textbox", { name: "Search settings" }).fill("CMake");
  await dialog.getByRole("treeitem", { name: "CMake", exact: true }).first().click();
  const presets = dialog.getByRole("region", { name: "CMake presets" });
  await expect(
    presets.getByRole("checkbox", { name: "Configure and build with presets instead of profiles" }),
  ).toBeVisible();
  // Turning preset mode on reveals the selection group (empty: the preview has no presets).
  await presets.getByRole("checkbox", { name: "Configure and build with presets instead of profiles" }).click();
  await expect(presets.getByRole("group", { name: "Configure presets" })).toBeVisible();
  await page.screenshot({ path: "docs/media/openclion-cmake-presets.png", animations: "disabled" });
});

test("Search Everywhere finds Generate Definition", async ({ workbench: page }) => {
  await openFromTree(page, "include/greeter.h");
  // The new C/C++ action is registered and searchable (running it needs the backend).
  await doubleShift(page);
  const dialog = page.getByRole("dialog", { name: "Search Everywhere" });
  await dialog.getByRole("combobox").fill("Generate Definition");
  await expect(dialog.getByRole("option", { name: /Generate Definition/ }).first()).toBeVisible();
});

test("Search Everywhere lists the C++ generation actions", async ({ workbench: page }) => {
  await doubleShift(page);
  const dialog = page.getByRole("dialog", { name: "Search Everywhere" });
  for (const name of ["Generate Override/Implement Methods", "Fix Missing Includes"]) {
    await dialog.getByRole("combobox").fill(name);
    await expect(dialog.getByRole("option", { name: new RegExp(name) }).first()).toBeVisible();
  }
});

test("Search Everywhere lists the Valgrind and sanitizer runs", async ({ workbench: page }) => {
  await doubleShift(page);
  const dialog = page.getByRole("dialog", { name: "Search Everywhere" });
  for (const name of ["Run with Valgrind Memcheck", "Run with Sanitizers"]) {
    await dialog.getByRole("combobox").fill(name);
    await expect(dialog.getByRole("option", { name: new RegExp(name) }).first()).toBeVisible();
  }
});

test("a CMake profile builds with sanitizers from Settings", async ({ workbench: page }) => {
  await page.keyboard.press("Meta+,");
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.getByRole("textbox", { name: "Search settings" }).fill("CMake");
  await dialog.getByRole("treeitem", { name: "CMake", exact: true }).first().click();
  const sanitizers = dialog.getByRole("checkbox", {
    name: "Build with AddressSanitizer and UndefinedBehaviorSanitizer",
  });
  await expect(sanitizers).not.toBeChecked();
  await sanitizers.click();
  await expect(dialog.getByLabel("CMake arguments")).toHaveValue(/-fsanitize=address,undefined/);
  await page.screenshot({ path: "docs/media/openclion-sanitizers.png", animations: "disabled" });
});
