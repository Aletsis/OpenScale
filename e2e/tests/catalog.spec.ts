import { expect, test } from "@playwright/test";

test("renders PLU catalog, switches EN/ES language, and adds a new PLU row", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Master PLU Catalog" })).toBeVisible();
  await expect(page.getByText("Online")).toBeVisible();
  await expect(page.getByText("POLLO FRESCO")).toBeVisible();
  await expect(page.getByText("$45.00")).toBeVisible();

  await page.getByRole("button", { name: /Language: EN/i }).click();
  await expect(page.getByRole("heading", { name: "Catálogo Maestro de PLUs" })).toBeVisible();
  await expect(page.getByText("En línea")).toBeVisible();

  await page.getByRole("button", { name: "Nuevo PLU" }).click();
  await expect(page.getByText("BISTEC DE RES")).toBeVisible();
  await expect(page.getByText("$189.00")).toBeVisible();
});
