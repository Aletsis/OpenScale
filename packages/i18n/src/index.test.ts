import { describe, expect, it } from "vitest";

import { SUPPORTED_LANGUAGES, initOpenScaleI18n, resources } from "./index.js";

describe("@openscale/i18n", () => {
  it("exposes English and Spanish dictionaries with matching top-level keys", () => {
    expect(SUPPORTED_LANGUAGES).toEqual(["en", "es"]);
    expect(Object.keys(resources.en.translation).sort()).toEqual(
      Object.keys(resources.es.translation).sort(),
    );
  });

  it("initializes i18next and switches languages between en and es", async () => {
    const instance = initOpenScaleI18n("en");
    expect(instance.isInitialized).toBe(true);
    expect(instance.t("nav.catalog")).toBe("PLU Catalog");

    await instance.changeLanguage("es");
    expect(instance.t("nav.catalog")).toBe("Catálogo PLU");

    await instance.changeLanguage("en");
    expect(instance.t("scales.discover")).toBe("Discover Scales");
  });
});
