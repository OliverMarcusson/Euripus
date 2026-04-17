import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Heart, Play } from "lucide-react";
import type { FavoriteCategoryEntry, FavoriteChannelEntry, FavoriteEntry } from "@euripus/shared";
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
import { useCategoryFavoriteMutation } from "@/hooks/use-category-favorite";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";
import { getFavorites, reorderFavorites } from "@/lib/api";
import { STANDARD_QUERY_STALE_TIME_MS } from "@/lib/query-cache";
import {
  formatArchiveDuration,
  formatEventChannelTitle,
} from "@/lib/utils";
import { useGuideNavigationStore } from "@/store/guide-navigation-store";

export function FavoritesPage() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const requestOpenCategory = useGuideNavigationStore(
    (state) => state.requestOpenCategory,
  );
  const favoritesQuery = useQuery({
    queryKey: ["favorites"],
    queryFn: getFavorites,
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
  });
  const favoriteMutation = useChannelFavoriteMutation();
  const categoryFavoriteMutation = useCategoryFavoriteMutation();
  const playMutation = useChannelPlaybackMutation();
  const reorderMutation = useMutation({
    mutationFn: reorderFavorites,
    onMutate: async (payload) => {
      await queryClient.cancelQueries({ queryKey: ["favorites"] });
      queryClient.setQueryData<FavoriteEntry[]>(["favorites"], (current) => {
        if (!current) {
          return current;
        }

        const categoriesById = new Map(
          current
            .filter((entry): entry is FavoriteCategoryEntry => entry.kind === "category")
            .map((entry) => [entry.category.id, entry]),
        );
        const channelsById = new Map(
          current
            .filter((entry): entry is FavoriteChannelEntry => entry.kind === "channel")
            .map((entry) => [entry.channel.id, entry]),
        );

        const nextCategories = payload.categoryIds
          .map((id, index) => {
            const entry = categoriesById.get(id);
            return entry ? { ...entry, order: index } : null;
          })
          .filter((entry): entry is FavoriteCategoryEntry => entry !== null);
        const nextChannels = payload.channelIds
          .map((id, index) => {
            const entry = channelsById.get(id);
            return entry ? { ...entry, order: index } : null;
          })
          .filter((entry): entry is FavoriteChannelEntry => entry !== null);

        return [...nextCategories, ...nextChannels];
      });
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: ["favorites"] });
    },
  });

  const favorites = favoritesQuery.data ?? [];
  const categoryFavorites = favorites.filter(
    (entry): entry is FavoriteCategoryEntry => entry.kind === "category",
  );
  const channelFavorites = favorites.filter(
    (entry): entry is FavoriteChannelEntry => entry.kind === "channel",
  );
  const savedCount = categoryFavorites.length + channelFavorites.length;

  function persistFavoriteOrder(
    nextCategoryFavorites: FavoriteCategoryEntry[],
    nextChannelFavorites: FavoriteChannelEntry[],
  ) {
    reorderMutation.mutate({
      categoryIds: nextCategoryFavorites.map((entry) => entry.category.id),
      channelIds: nextChannelFavorites.map((entry) => entry.channel.id),
    });
  }

  function moveCategory(index: number, direction: -1 | 1) {
    const nextCategories = moveEntry(categoryFavorites, index, direction);
    if (nextCategories === categoryFavorites) {
      return;
    }
    persistFavoriteOrder(nextCategories, channelFavorites);
  }

  function moveChannel(index: number, direction: -1 | 1) {
    const nextChannels = moveEntry(channelFavorites, index, direction);
    if (nextChannels === channelFavorites) {
      return;
    }
    persistFavoriteOrder(categoryFavorites, nextChannels);
  }

  return (
    <div className="flex flex-col gap-5 sm:gap-6">
      <PageHeader
        title="Favorites"
        actions={(
          <Button variant="outline" onClick={() => void navigate({ to: "/favorites/ppv" })}>
            Open PPV favorites
          </Button>
        )}
        meta={<Badge variant="accent">{savedCount} saved</Badge>}
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
                  <Heart aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>No favorites yet</EmptyTitle>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!favoritesQuery.isPending && favorites.length ? (
        <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-5 sm:pb-0">
            <CardTitle>Saved items</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {categoryFavorites.map((entry, index) => (
              <FavoriteCategoryRow
                key={entry.category.id}
                entry={entry}
                index={index}
                pending={
                  categoryFavoriteMutation.isPending &&
                  categoryFavoriteMutation.variables?.id === entry.category.id
                }
                onOpen={() => {
                  requestOpenCategory(entry.category.id);
                  void navigate({ to: "/guide" });
                }}
                onToggleFavorite={() =>
                  categoryFavoriteMutation.mutate(entry.category)
                }
                canMoveUp={index > 0}
                canMoveDown={index < categoryFavorites.length - 1}
                onMoveUp={() => moveCategory(index, -1)}
                onMoveDown={() => moveCategory(index, 1)}
                reorderPending={reorderMutation.isPending}
              />
            ))}
            {channelFavorites.map(({ channel, program }, index) => {
              const displayChannelName = formatEventChannelTitle(channel.name, {
                referenceStartAt: program?.startAt,
              });

              return (
                <div key={channel.id} className="group">
                {index > 0 || categoryFavorites.length > 0 ? <Separator /> : null}
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
                        </div>
                      </div>
                    </div>
                    <div className="hidden shrink-0 items-center gap-2 pt-1 sm:flex">
                      <MoveButtons
                        onMoveUp={() => moveChannel(index, -1)}
                        onMoveDown={() => moveChannel(index, 1)}
                        canMoveUp={index > 0}
                        canMoveDown={index < channelFavorites.length - 1}
                        disabled={reorderMutation.isPending}
                      />
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
                        onClick={() => favoriteMutation.mutate(channel)}
                        disabled={
                          favoriteMutation.isPending &&
                          favoriteMutation.variables?.id === channel.id
                        }
                      >
                        <Heart data-icon="inline-start" className="fill-current opacity-70" />
                        Unfavorite
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
                      canMoveDown={index < channelFavorites.length - 1}
                      disabled={reorderMutation.isPending}
                    />
                    <Button
                      variant="secondary"
                      className="flex-1 bg-secondary/50 shadow-sm"
                      onClick={() => favoriteMutation.mutate(channel)}
                      disabled={
                        favoriteMutation.isPending &&
                        favoriteMutation.variables?.id === channel.id
                      }
                    >
                      <Heart data-icon="inline-start" className="fill-current opacity-70" />
                      Unfavorite
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

function FavoriteCategoryRow({
  entry,
  index,
  pending,
  onOpen,
  onToggleFavorite,
  canMoveUp,
  canMoveDown,
  onMoveUp,
  onMoveDown,
  reorderPending,
}: {
  entry: FavoriteCategoryEntry;
  index: number;
  pending: boolean;
  onOpen: () => void;
  onToggleFavorite: () => void;
  canMoveUp: boolean;
  canMoveDown: boolean;
  onMoveUp: () => void;
  onMoveDown: () => void;
  reorderPending: boolean;
}) {
  const { category } = entry;

  return (
    <div className="group">
      {index > 0 ? <Separator /> : null}
      <div className="flex flex-col gap-4 p-4 transition-colors hover:bg-muted/30 sm:gap-5 sm:p-5">
        <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <h2 className="min-w-0 break-words text-base font-semibold tracking-tight sm:text-lg">
                {category.name}
              </h2>
              <Badge variant="accent">Category</Badge>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="outline">{category.channelCount} channels</Badge>
              <Badge variant={category.liveNowCount ? "live" : "outline"}>
                {category.liveNowCount} live now
              </Badge>
            </div>
          </div>
          <div className="hidden shrink-0 items-center gap-2 pt-1 sm:flex">
            <MoveButtons
              onMoveUp={onMoveUp}
              onMoveDown={onMoveDown}
              canMoveUp={canMoveUp}
              canMoveDown={canMoveDown}
              disabled={reorderPending}
            />
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
              onClick={onToggleFavorite}
              disabled={pending}
            >
              <Heart data-icon="inline-start" className="fill-current opacity-70" />
              Unfavorite
            </Button>
            <Button size="sm" className="min-w-24 shadow-sm" onClick={onOpen}>
              Open
            </Button>
          </div>
        </div>
        <div className="flex w-full items-center gap-2 sm:hidden">
          <MoveButtons
            onMoveUp={onMoveUp}
            onMoveDown={onMoveDown}
            canMoveUp={canMoveUp}
            canMoveDown={canMoveDown}
            disabled={reorderPending}
          />
          <Button
            variant="secondary"
            className="flex-1 bg-secondary/50 shadow-sm"
            onClick={onToggleFavorite}
            disabled={pending}
          >
            <Heart data-icon="inline-start" className="fill-current opacity-70" />
            Unfavorite
          </Button>
          <Button className="flex-1 shadow-sm" onClick={onOpen}>
            Open
          </Button>
        </div>
      </div>
    </div>
  );
}

