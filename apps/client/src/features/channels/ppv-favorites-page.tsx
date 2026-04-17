import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Heart, Play, Tv } from "lucide-react";
import type { FavoriteChannelEntry } from "@euripus/shared";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import {
  FavoriteProgramDetails,
  MoveButtons,
  moveEntry,
} from "@/features/channels/favorites-shared";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";
import { usePpvFavoriteMutation } from "@/hooks/use-ppv-favorite";
import {
  getPpvFavorites,
  reorderPpvFavorites,
} from "@/lib/api";
import { STANDARD_QUERY_STALE_TIME_MS } from "@/lib/query-cache";
import {
  formatArchiveDuration,
  formatEventChannelTitle,
} from "@/lib/utils";

export function PpvFavoritesPage() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const favoritesQuery = useQuery({
    queryKey: ["favorites", "ppv"],
    queryFn: getPpvFavorites,
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
  });
  const favoriteMutation = usePpvFavoriteMutation();
  const playMutation = useChannelPlaybackMutation();
  const reorderMutation = useMutation({
    mutationFn: reorderPpvFavorites,
    onMutate: async (payload) => {
      await queryClient.cancelQueries({ queryKey: ["favorites", "ppv"] });
      queryClient.setQueryData<FavoriteChannelEntry[]>(["favorites", "ppv"], (current) => {
        if (!current) {
          return current;
        }

        const channelsById = new Map(current.map((entry) => [entry.channel.id, entry]));
        return payload.channelIds
          .map((id, index) => {
            const entry = channelsById.get(id);
            return entry ? { ...entry, order: index } : null;
          })
          .filter((entry): entry is FavoriteChannelEntry => entry !== null);
      });
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: ["favorites", "ppv"] });
    },
  });

  const favorites = favoritesQuery.data ?? [];

  function persistFavoriteOrder(nextFavorites: FavoriteChannelEntry[]) {
    reorderMutation.mutate({
      channelIds: nextFavorites.map((entry) => entry.channel.id),
    });
  }

  function moveChannel(index: number, direction: -1 | 1) {
    const nextChannels = moveEntry(favorites, index, direction);
    if (nextChannels === favorites) {
      return;
    }
    persistFavoriteOrder(nextChannels);
  }

  return (
    <div className="flex flex-col gap-5 sm:gap-6">
      <PageHeader
        title="PPV Favorites"
        actions={(
          <Button variant="outline" onClick={() => void navigate({ to: "/favorites" })}>
            Open regular favorites
          </Button>
        )}
        meta={<Badge variant="accent">{favorites.length} saved</Badge>}
      />

      {favoritesQuery.isPending ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="flex flex-col gap-3 px-0 pb-0 pt-0 sm:p-5 sm:pt-5">
            {Array.from({ length: 4 }).map((_, index) => (
              <div
                key={index}
                className="flex items-center gap-4 rounded-xl border border-border/70 p-4"
              >
                <Skeleton className="size-11 rounded-2xl" />
                <div className="flex flex-1 flex-col gap-2">
                  <Skeleton className="h-4 w-40" />
                  <Skeleton className="h-4 w-28" />
                </div>
                <Skeleton className="h-9 w-28 rounded-lg" />
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}

      {!favoritesQuery.isPending && !favorites.length ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Tv aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>No PPV favorites yet</EmptyTitle>
                <p className="text-sm text-muted-foreground">
                  Save temporary PPV event channels here so they stay separate from your regular favorites.
                </p>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!favoritesQuery.isPending && favorites.length ? (
        <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-5 sm:pb-0">
            <CardTitle>Saved PPV events</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {favorites.map(({ channel, program }, index) => {
              const displayChannelName = formatEventChannelTitle(channel.name, {
                referenceStartAt: program?.startAt,
              });

              return (
                <div key={channel.id} className="group">
                  {index > 0 ? <Separator /> : null}
                  <div className="flex flex-col gap-4 p-4 transition-colors hover:bg-muted/30 sm:gap-5 sm:p-5">
                    <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
                      <div className="flex min-w-0 flex-1 items-start gap-3 sm:gap-4">
                        <ChannelAvatar
                          name={displayChannelName}
                          logoUrl={channel.logoUrl}
                          className="h-12 w-12 shrink-0 rounded-xl ring-1 ring-border/10 sm:h-14 sm:w-14 sm:rounded-2xl"
                          fallbackClassName="rounded-xl sm:rounded-2xl"
                        />
                        <div className="flex min-w-0 flex-1 flex-col gap-1.5 pt-0.5">
                          <div className="flex min-w-0 flex-wrap items-center gap-2">
                            <h2 className="min-w-0 break-words text-base font-semibold tracking-tight sm:text-lg">
                              {displayChannelName}
                            </h2>
                            <Badge variant="accent">PPV</Badge>
                            {channel.streamExtension ? (
                              <Badge variant="outline" className="border-transparent bg-background/50 text-[10px] uppercase">
                                {channel.streamExtension}
                              </Badge>
                            ) : null}
                          </div>
                          <div className="flex flex-wrap items-center gap-1.5 sm:gap-2">
                            {channel.categoryName ? (
                              <Badge variant="outline" className="h-5 border-transparent bg-secondary/40 px-1.5 py-0 text-[11px] font-normal hover:bg-secondary/40">
                                {channel.categoryName}
                              </Badge>
                            ) : null}
                            {channel.hasCatchup ? (
                              <Badge variant="live" className="h-5 px-1.5 py-0 text-[10px] font-medium tracking-wide">
                                Catch-up
                              </Badge>
                            ) : null}
                            {channel.archiveDurationHours ? (
                              <Badge className="h-5 bg-primary/10 px-1.5 py-0 text-[10px] font-medium text-primary hover:bg-primary/20 hover:text-primary">
                                {formatArchiveDuration(channel.archiveDurationHours)}
                              </Badge>
                            ) : null}
                            {channel.isFavorite ? <Badge variant="outline">Also in favorites</Badge> : null}
                          </div>
                        </div>
                      </div>
                      <div className="hidden shrink-0 items-center gap-2 pt-1 sm:flex">
                        <MoveButtons
                          onMoveUp={() => moveChannel(index, -1)}
                          onMoveDown={() => moveChannel(index, 1)}
                          canMoveUp={index > 0}
                          canMoveDown={index < favorites.length - 1}
                          disabled={reorderMutation.isPending}
                        />
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
                          onClick={() => favoriteMutation.mutate(channel)}
                          disabled={
                            favoriteMutation.isPending
                            && favoriteMutation.variables?.id === channel.id
                          }
                        >
                          <Heart data-icon="inline-start" className="fill-current opacity-70" />
                          Remove PPV
                        </Button>
                        <Button
                          size="sm"
                          className="min-w-24 shadow-sm"
                          onClick={() => playMutation.mutate(channel.id)}
                          disabled={playMutation.isPending}
                        >
                          <Play data-icon="inline-start" />
                          Play
                        </Button>
                      </div>
                    </div>
                    <div className="flex w-full items-center gap-2 sm:hidden">
                      <MoveButtons
                        onMoveUp={() => moveChannel(index, -1)}
                        onMoveDown={() => moveChannel(index, 1)}
                        canMoveUp={index > 0}
                        canMoveDown={index < favorites.length - 1}
                        disabled={reorderMutation.isPending}
                      />
                      <Button
                        variant="secondary"
                        className="flex-1 bg-secondary/50 shadow-sm"
                        onClick={() => favoriteMutation.mutate(channel)}
                        disabled={
                          favoriteMutation.isPending
                          && favoriteMutation.variables?.id === channel.id
                        }
                      >
                        <Heart data-icon="inline-start" className="fill-current opacity-70" />
                        Remove PPV
                      </Button>
                      <Button
                        className="flex-1 shadow-sm"
                        onClick={() => playMutation.mutate(channel.id)}
                        disabled={playMutation.isPending}
                      >
                        <Play data-icon="inline-start" />
                        Play
                      </Button>
                    </div>
                    {program ? (
                      <div className="rounded-xl border border-border/40 bg-secondary/20 p-3.5 sm:p-4">
                        <FavoriteProgramDetails program={program} />
                      </div>
                    ) : null}
                  </div>
                </div>
              );
            })}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
