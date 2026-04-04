import { type InfiniteData, useMutation, useQueryClient } from "@tanstack/react-query";
import type { Channel, GuideCategoryResponse, RecentChannel, SearchResults } from "@euripus/shared";
import { addFavorite, removeFavorite } from "@/lib/api";

function withFavorite(channel: Channel, isFavorite: boolean): Channel {
  return { ...channel, isFavorite };
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

      queryClient.setQueryData<Channel[]>(["favorites"], (current) => {
        if (!current) {
          return current;
        }

        if (nextIsFavorite) {
          const existing = current.some((favorite) => favorite.id === channel.id);
          return existing ? current.map((favorite) => (favorite.id === channel.id ? nextChannel : favorite)) : [nextChannel, ...current];
        }

        return current.filter((favorite) => favorite.id !== channel.id);
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

      queryClient.setQueriesData<SearchResults>({ queryKey: ["search"] }, (current) => {
        if (!current) {
          return current;
        }

        return {
          ...current,
          channels: current.channels.map((item) => (item.id === channel.id ? withFavorite(item, nextIsFavorite) : item)),
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
