import { type InfiniteData, useMutation, useQueryClient } from "@tanstack/react-query";
import type {
  Channel,
  ChannelSearchResults,
  FavoriteChannelEntry,
  FavoriteEntry,
  GuideCategoryResponse,
  RecentChannel,
} from "@euripus/shared";
import { addPpvFavorite, removePpvFavorite } from "@/lib/api";

function withPpvFavorite(channel: Channel, isPpvFavorite: boolean): Channel {
  return { ...channel, isPpvFavorite };
}

export function usePpvFavoriteMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (channel: Channel) =>
      channel.isPpvFavorite
        ? removePpvFavorite(channel.id)
        : addPpvFavorite(channel.id),
    onMutate: async (channel) => {
      const nextIsPpvFavorite = !channel.isPpvFavorite;
      const nextChannel = withPpvFavorite(channel, nextIsPpvFavorite);

      await Promise.all([
        queryClient.cancelQueries({ queryKey: ["favorites"] }),
        queryClient.cancelQueries({ queryKey: ["favorites", "ppv"] }),
        queryClient.cancelQueries({ queryKey: ["guide"] }),
        queryClient.cancelQueries({ queryKey: ["search"] }),
        queryClient.cancelQueries({ queryKey: ["recents"] }),
      ]);

      queryClient.setQueryData<FavoriteEntry[]>(["favorites"], (current) => {
        if (!current) {
          return current;
        }

        return current.map((favorite) =>
          favorite.kind === "channel" && favorite.channel.id === channel.id
            ? { ...favorite, channel: withPpvFavorite(favorite.channel, nextIsPpvFavorite) }
            : favorite,
        );
      });

      queryClient.setQueryData<FavoriteChannelEntry[]>(["favorites", "ppv"], (current) => {
        if (!current) {
          return current;
        }

        if (nextIsPpvFavorite) {
          const existing = current.some((favorite) => favorite.channel.id === channel.id);
          if (existing) {
            return current.map((favorite) =>
              favorite.channel.id === channel.id
                ? { ...favorite, channel: nextChannel }
                : favorite,
            );
          }

          return [
            ...current,
            {
              kind: "channel",
              channel: nextChannel,
              program: null,
              order:
                current.length > 0
                  ? Math.max(...current.map((entry) => entry.order)) + 1
                  : 0,
            },
          ];
        }

        return current.filter((favorite) => favorite.channel.id !== channel.id);
      });

      queryClient.setQueriesData<InfiniteData<GuideCategoryResponse, number>>(
        { queryKey: ["guide", "category"] },
        (current) => {
          if (!current) {
            return current;
          }

          return {
            ...current,
            pages: current.pages.map((page) => ({
              ...page,
              entries: page.entries.map((entry) =>
                entry.channel.id === channel.id
                  ? { ...entry, channel: withPpvFavorite(entry.channel, nextIsPpvFavorite) }
                  : entry,
              ),
            })),
          };
        },
      );

      queryClient.setQueriesData<InfiniteData<ChannelSearchResults, number>>(
        { queryKey: ["search", "channels"] },
        (current) => {
          if (!current) {
            return current;
          }

          return {
            ...current,
            pages: current.pages.map((page) => ({
              ...page,
              items: page.items.map((item) =>
                item.id === channel.id ? withPpvFavorite(item, nextIsPpvFavorite) : item,
              ),
            })),
          };
        },
      );

      queryClient.setQueryData<RecentChannel[]>(["recents"], (current) => {
        if (!current) {
          return current;
        }

        return current.map((recent) =>
          recent.channel.id === channel.id
            ? { ...recent, channel: withPpvFavorite(recent.channel, nextIsPpvFavorite) }
            : recent,
        );
      });
    },
    onError: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
        queryClient.invalidateQueries({ queryKey: ["favorites", "ppv"] }),
        queryClient.invalidateQueries({ queryKey: ["guide", "category"] }),
        queryClient.invalidateQueries({ queryKey: ["search", "channels"] }),
        queryClient.invalidateQueries({ queryKey: ["recents"] }),
      ]);
    },
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
        queryClient.invalidateQueries({ queryKey: ["favorites", "ppv"] }),
      ]);
    },
  });
}
