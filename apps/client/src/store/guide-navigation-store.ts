import { create } from "zustand";

type GuideNavigationState = {
  pendingOpenCategoryId: string | null;
  requestOpenCategory: (categoryId: string) => void;
  clearPendingOpenCategory: () => void;
};

export const useGuideNavigationStore = create<GuideNavigationState>((set) => ({
  pendingOpenCategoryId: null,
  requestOpenCategory: (categoryId) => set({ pendingOpenCategoryId: categoryId }),
  clearPendingOpenCategory: () => set({ pendingOpenCategoryId: null }),
}));
