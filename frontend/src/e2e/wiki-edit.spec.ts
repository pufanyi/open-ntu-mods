import { expect, test } from "@playwright/test";

test("dev user can edit a section and see history", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Email").fill("student@e.ntu.edu.sg");
  await page.getByLabel("Display name").fill("Demo Student");
  await page.getByLabel("Role").selectOption("verified_user");
  await page.getByRole("button", { name: "Login as dev user" }).click();

  await page.getByText("SC2001").click();
  await page.getByText("AY2025/26 Sem 1").click();
  await page.getByText("Assessment").click();
  await page.getByRole("link", { name: "Edit" }).click();

  await page.getByLabel("Edit summary").fill("E2E assessment update");
  await page.getByLabel("Markdown").fill("E2E public assessment note.");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("E2E public assessment note.")).toBeVisible();
  await page.getByRole("link", { name: "History" }).click();
  await expect(page.getByText("E2E assessment update")).toBeVisible();
});
