import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import type { GuidePreferences } from "@euripus/shared";
import { getGuide, getGuidePreferences, saveGuidePreferences } from "@/lib/api";
import { STANDARD_QUERY_STALE_TIME_MS } from "@/lib/query-cache";
import { useGuideNavigationStore } from "@/store/guide-navigation-store";

export function useGuidePageState() {
  const queryClient = useQueryClient();
  const pendingOpenCategoryId = useGuideNavigationStore(
    (state) => state.pendingOpenCategoryId,
  );
  const clearPendingOpenCategory = useGuideNavigationStore(
    (state) => state.clearPendingOpenCategory,
  );
  const [openCategories, setOpenCategories] = useState<string[]>([]);
  const [forcedVisibleCategoryId, setForcedVisibleCategoryId] = useState<string | null>(null);
  const [filterInput, setFilterInput] = useState("");
  const [appliedFilter, setAppliedFilter] = useState("");
  const [showOnlyChannelsWithEpg, setShowOnlyChannelsWithEpg] = useState(false);

  const guideQuery = useQuery({
    queryKey: ["guide", "overview", { withEpgOnly: showOnlyChannelsWithEpg }],
    queryFn: () => getGuide(showOnlyChannelsWithEpg),
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
  });
  const preferencesQuery = useQuery({
    queryKey: ["guide", "preferences"],
    queryFn: getGuidePreferences,
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
  });
  const savePreferencesMutation = useMutation({
    mutationFn: saveGuidePreferences,
    onMutate: async (nextPreferences) => {
      await queryClient.cancelQueries({ queryKey: ["guide", "preferences"] });
      const previousPreferences = queryClient.getQueryData<GuidePreferences>([
        "guide",
        "preferences",
      ]);
      queryClient.setQueryData<GuidePreferences>(
        ["guide", "preferences"],
        nextPreferences,
      );
      return { previousPreferences };
    },
    onError: (_error, _variables, context) => {
      if (context?.previousPreferences) {
        queryClient.setQueryData(
          ["guide", "preferences"],
          context.previousPreferences,
        );
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["guide", "preferences"],
      });
    },
  });

  const categories = guideQuery.data?.categories ?? [];
  const liveCount = categories.reduce(
    (sum, category) => sum + category.liveNowCount,
    0,
  );
  const savedCategoryIds = preferencesQuery.data?.includedCategoryIds ?? [];
  const availableCategoryIds = useMemo(
    () => new Set(categories.map((category) => category.id)),
    [categories],
  );
  const validSelectedCategoryIds = useMemo(
    () =>
      savedCategoryIds.filter((categoryId) =>
        availableCategoryIds.has(categoryId),
      ),
    [availableCategoryIds, savedCategoryIds],
  );
  const validSelectedCategoryIdsSet = useMemo(
    () => new Set(validSelectedCategoryIds),
    [validSelectedCategoryIds],
  );
  const normalizedAppliedFilter = appliedFilter.trim().toLowerCase();
  const shouldApplyFilter = normalizedAppliedFilter.length >= 2;
  const chooserCategories = useMemo(() => {
    if (!shouldApplyFilter) {
      return categories;
    }

    return categories.filter((category) =>
      category.name.toLowerCase().includes(normalizedAppliedFilter),
    );
  }, [categories, normalizedAppliedFilter, shouldApplyFilter]);
  const selectedCategories = useMemo(() => {
    if (!validSelectedCategoryIds.length && !forcedVisibleCategoryId) {
      return categories;
    }

    return categories.filter(
      (category) =>
        validSelectedCategoryIdsSet.has(category.id)
        || category.id === forcedVisibleCategoryId,
    );
  }, [
    categories,
    forcedVisibleCategoryId,
    validSelectedCategoryIds.length,
    validSelectedCategoryIdsSet,
  ]);
  const visibleCategories = useMemo(() => {
    if (!shouldApplyFilter) {
      return selectedCategories;
    }

    const filteredCategories = selectedCategories.filter((category) =>
      category.name.toLowerCase().includes(normalizedAppliedFilter),
    );

    if (!forcedVisibleCategoryId) {
      return filteredCategories;
    }

    return selectedCategories.filter(
      (category) =>
        category.id === forcedVisibleCategoryId
        || filteredCategories.some((entry) => entry.id === category.id),
    );
  }, [
    forcedVisibleCategoryId,
    normalizedAppliedFilter,
    selectedCategories,
    shouldApplyFilter,
  ]);

  useEffect(() => {
    const requestedCategoryId = pendingOpenCategoryId;
    if (!requestedCategoryId || !categories.length) {
      return;
    }

    const categoryExists = categories.some(
      (category) => category.id === requestedCategoryId,
    );
    if (!categoryExists) {
      clearPendingOpenCategory();
      return;
    }

    setOpenCategories((current) =>
      current.includes(requestedCategoryId)
        ? current
        : [...current, requestedCategoryId],
    );
    setForcedVisibleCategoryId(requestedCategoryId);
    clearPendingOpenCategory();
  }, [categories, clearPendingOpenCategory, pendingOpenCategoryId]);

  function toggleCategory(categoryId: string, nextOpen: boolean) {
    setOpenCategories((current) =>
      nextOpen
        ? [...new Set([...current, categoryId])]
        : current.filter((id) => id !== categoryId),
    );
  }

  function updateIncludedCategoryIds(includedCategoryIds: string[]) {
    savePreferencesMutation.mutate({ includedCategoryIds });
  }

  function applyFilter() {
    const nextFilter = filterInput.trim();
    setAppliedFilter(nextFilter.length >= 2 ? nextFilter : "");
  }

  function toggleIncludedCategory(categoryId: string) {
    if (savedCategoryIds.includes(categoryId)) {
      updateIncludedCategoryIds(
        savedCategoryIds.filter((id) => id !== categoryId),
      );
      return;
    }

    updateIncludedCategoryIds([...savedCategoryIds, categoryId]);
  }

  async function refreshGuide() {
    await queryClient.invalidateQueries({ queryKey: ["guide"] });
  }

  return {
    appliedFilter,
    applyFilter,
    categories,
    chooserCategories,
    filterInput,
    guideQuery,
    liveCount,
    openCategories,
    preferencesQuery,
    refreshGuide,
    savePreferencesMutation,
    setFilterInput,
    setShowOnlyChannelsWithEpg,
    showOnlyChannelsWithEpg,
    shouldApplyFilter,
    toggleCategory,
    toggleIncludedCategory,
    updateIncludedCategoryIds,
    validSelectedCategoryIds,
    visibleCategories,
  };
}
