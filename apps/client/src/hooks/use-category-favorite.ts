import { useMutation, useQueryClient } from "@tanstack/react-query";
import type {
  FavoriteCategoryEntry,
  FavoriteChannelEntry,
  FavoriteEntry,
  GuideResponse,
  GuideCategorySummary,
} from "@euripus/shared";
import { addCategoryFavorite, removeCategoryFavorite } from "@/lib/api";

function withFavorite(
  category: GuideCategorySummary,
  isFavorite: boolean,
): GuideCategorySummary {
  return { ...category, isFavorite };
}

export function useCategoryFavoriteMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (category: GuideCategorySummary) =>
      category.isFavorite
        ? removeCategoryFavorite(category.id)
        : addCategoryFavorite(category.id),
    onMutate: async (category) => {
      const nextIsFavorite = !category.isFavorite;
      const nextCategory = withFavorite(category, nextIsFavorite);

      await Promise.all([
        queryClient.cancelQueries({ queryKey: ["guide", "overview"] }),
        queryClient.cancelQueries({ queryKey: ["favorites"] }),
      ]);

      queryClient.setQueryData<GuideResponse>(["guide", "overview"], (current) => {
        if (!current) {
          return current;
        }

        return {
          ...current,
          categories: current.categories.map((entry) =>
            entry.id === category.id ? nextCategory : entry,
          ),
        };
      });

      queryClient.setQueryData<FavoriteEntry[]>(["favorites"], (current) => {
        if (!current) {
          return current;
        }

        const categoryEntries = current.filter(
          (entry): entry is FavoriteCategoryEntry => entry.kind === "category",
        );
        const channelEntries = current.filter(
          (entry): entry is FavoriteChannelEntry => entry.kind === "channel",
        );

        if (nextIsFavorite) {
          const existing = categoryEntries.some(
            (entry) => entry.category.id === category.id,
          );
          const nextCategoryEntries = existing
            ? categoryEntries.map((entry) =>
                entry.category.id === category.id
                  ? { kind: "category" as const, category: nextCategory, order: entry.order }
                  : entry,
              )
            : [
                ...categoryEntries,
                {
                  kind: "category" as const,
                  category: nextCategory,
                  order:
                    categoryEntries.length > 0
                      ? Math.max(...categoryEntries.map((entry) => entry.order)) + 1
                      : 0,
                },
              ];

          return [...nextCategoryEntries, ...channelEntries];
        }

        return [
          ...categoryEntries.filter((entry) => entry.category.id !== category.id),
          ...channelEntries,
        ];
      });
    },
    onError: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["guide", "overview"] }),
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
      ]);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["favorites"] });
    },
  });
}
