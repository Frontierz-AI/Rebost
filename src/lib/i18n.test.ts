import { describe, expect, it } from "vitest";
import { APP_LOCALES, DRAFT_LOCALES, catalogKeys, catalogsByLocale, t } from "./i18n.svelte";

describe("catalogs", () => {
  it("keeps the same keys in every shipped catalog", () => {
    const en = catalogKeys(catalogsByLocale.en).sort();
    for (const locale of APP_LOCALES) {
      expect(catalogKeys(catalogsByLocale[locale]).sort(), locale).toEqual(en);
    }
  });

  it("marks draft catalogs for a native-speaker check", () => {
    for (const locale of DRAFT_LOCALES) {
      const review = (catalogsByLocale[locale] as { _review?: string })._review;
      expect(review, locale).toMatch(/AI-generated/);
      expect(review, locale).toContain(`locales/${locale}.json`);
    }
  });

  it("looks up nested keys and interpolates", () => {
    expect(t("errors.shelfExists", { name: "Harbor" })).toBe(
      'A Shelf called "Harbor" already exists.',
    );
  });

  it("translates Chat, Shelves, and Recipes in the sidebar and View menu", () => {
    expect(catalogsByLocale.es.nav.chat).toBe("Chats");
    expect(catalogsByLocale.es.nav.shelves).toBe("Estantes");
    expect(catalogsByLocale.es.nav.recipes).toBe("Recetas");
    expect(catalogsByLocale.ca.nav.chat).toBe("Xat");
    expect(catalogsByLocale.ca.nav.shelves).toBe("Estants");
    expect(catalogsByLocale.ca.nav.recipes).toBe("Receptes");
    expect(catalogsByLocale.es.menu.chat).toBe(catalogsByLocale.es.nav.chat);
    expect(catalogsByLocale.ca.menu.recipes).toBe(catalogsByLocale.ca.nav.recipes);
    for (const locale of APP_LOCALES) {
      const nav = catalogsByLocale[locale].nav;
      const menu = catalogsByLocale[locale].menu;
      expect(menu.chat, locale).toBe(nav.chat);
      expect(menu.shelves, locale).toBe(nav.shelves);
      expect(menu.recipes, locale).toBe(nav.recipes);
    }
  });

  it("keeps this Shelf as a whole phrase in shipped Recipe prompts", () => {
    const needsShelf = /\bthis shelf\b/i;
    const recipes = catalogsByLocale.en.defaults.recipes;
    const shelfIds = Object.entries(recipes)
      .filter(([, rec]) => needsShelf.test(rec.prompt))
      .map(([id]) => id);
    expect(shelfIds.length).toBeGreaterThan(0);
    for (const locale of APP_LOCALES) {
      const recs = catalogsByLocale[locale].defaults.recipes;
      for (const id of shelfIds) {
        const rec = recs[id as keyof typeof recs];
        expect(needsShelf.test(rec.prompt), `${locale} ${id}`).toBe(true);
      }
    }
  });
});
