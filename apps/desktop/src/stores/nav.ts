import { create } from "zustand";

export type Page = "home" | "versions" | "settings";

const ORDER: Record<Page, number> = { home: 0, versions: 1, settings: 2 };

type NavState = {
  page: Page;
  direction: 1 | -1;
  go: (p: Page) => void;
};

export const useNav = create<NavState>((set) => ({
  page: "home",
  direction: 1,
  go: (page) =>
    set((s) => ({ page, direction: ORDER[page] > ORDER[s.page] ? 1 : -1 })),
}));
