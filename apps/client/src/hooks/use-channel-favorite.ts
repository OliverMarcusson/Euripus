import { type InfiniteData, useMutation, useQueryClient } from "@tanstack/react-query";
import type {
  Channel,
  ChannelSearchResults,
  FavoriteEntry,
  FavoriteChannelEntry,
  GuideCategoryResponse,
  RecentChannel,
} from "@euripus/shared";
import { addFavorite, removeFavorite } from "@/lib/api";

function withFavorite(channel: Channel, isFavorite: boolean): Channel {
  return { ...channel, isFavorite };
}

function splitFavorites(entries: FavoriteEntry[]) {
  const categoryEntries = entries.filter((entry) => entry.kind === "category");
  const channelEntries = entries.filter(
    (entry): entry is FavoriteChannelEntry => entry.kind === "channel",
  );

  return { categoryEntries, channelEntries };
}

export function useChannelFavoriteMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (channel: Channel) => (channel.isFavorite ? removeFavorite(channel.id) : addFavorite(channel.id)),
    onMutate: async (channel) => {
      const nextIsFavorite = !channel.isFavorite;
      const nextChannel = withFavorite(channel, nextIsFavorite);

      await Promise.all([
        queryClient.cancelQueries({ queryKey: ["favorites"] }),
        queryClient.cancelQueries({ queryKey: ["guide"] }),
        queryClient.cancelQueries({ queryKey: ["search"] }),
        queryClient.cancelQueries({ queryKey: ["recents"] }),
      ]);

      queryClient.setQueryData<FavoriteEntry[]>(["favorites"], (current) => {
        if (!current) {
          return current;
        }

        const { categoryEntries, channelEntries } = splitFavorites(current);

        if (nextIsFavorite) {
          const existing = channelEntries.some(
            (favorite) => favorite.channel.id === channel.id,
          );
          const nextChannelEntries = existing
            ? channelEntries.map((favorite) =>
                favorite.channel.id === channel.id
                  ? { ...favorite, channel: nextChannel }
                  : favorite,
              )
            : [{ kind: "channel" as const, channel: nextChannel, program: null }, ...channelEntries];

          return [...categoryEntries, ...nextChannelEntries];
        }

        return [
          ...categoryEntries,
          ...channelEntries.filter((favorite) => favorite.channel.id !== channel.id),
        ];
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
                entry.channel.id === channel.id ? { ...entry, channel: withFavorite(entry.channel, nextIsFavorite) } : entry,
              ),
            })),
          };
        },
      );

      queryClient.setQueriesData<InfiniteData<ChannelSearchResults, number>>({ queryKey: ["search", "channels"] }, (current) => {
        if (!current) {
          return current;
        }

        return {
          ...current,
          pages: current.pages.map((page) => ({
            ...page,
            items: page.items.map((item) => (item.id === channel.id ? withFavorite(item, nextIsFavorite) : item)),
          })),
        };
      });

      queryClient.setQueryData<RecentChannel[]>(["recents"], (current) => {
        if (!current) {
          return current;
        }

        return current.map((recent) =>
          recent.channel.id === channel.id ? { ...recent, channel: withFavorite(recent.channel, nextIsFavorite) } : recent,
        );
      });
    },
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
        queryClient.invalidateQueries({ queryKey: ["search"] }),
        queryClient.invalidateQueries({ queryKey: ["recents"] }),
      ]);
    },
  });
}
