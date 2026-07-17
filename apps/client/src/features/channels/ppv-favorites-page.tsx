import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Heart, Play, Search, Sparkles, Tv } from "lucide-react";
import type { FormEvent } from "react";
import { useState } from "react";
import type { AiPpvSearchResult, FavoriteChannelEntry } from "@euripus/shared";
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
  searchAiPpv,
} from "@/lib/api";
import { STANDARD_QUERY_STALE_TIME_MS } from "@/lib/query-cache";
import {
  cn,
  formatArchiveDuration,
  formatEventChannelTitle,
  getEventChannelPlaybackState,
  getProgramPlaybackState,
  shouldShowChannelForPpvDateFilter,
} from "@/lib/utils";
import { useChannelSettingsStore } from "@/store/channel-settings-store";

function isLiveFavorite(entry: FavoriteChannelEntry, now: Date) {
  if (entry.program) {
    return getProgramPlaybackState(entry.program, now.getTime()) === "live";
  }

  return getEventChannelPlaybackState(entry.channel.name, { now }) === "live";
}

function splitPpvFavorites(entries: FavoriteChannelEntry[], now: Date) {
  const liveFavorites: FavoriteChannelEntry[] = [];
  const otherFavorites: FavoriteChannelEntry[] = [];

  for (const entry of entries) {
    if (isLiveFavorite(entry, now)) {
      liveFavorites.push(entry);
      continue;
    }

    otherFavorites.push(entry);
  }

  return {
    liveFavorites,
    displayedFavorites: [...liveFavorites, ...otherFavorites],
  };
}

const ppvLiveBadgeClassName =
  "gap-1.5 border-transparent bg-gradient-to-r from-primary/25 via-fuchsia-500/15 to-violet-500/15 text-primary ring-primary/30 shadow-[0_0_24px_rgba(168,85,247,0.18)]";

const ppvGradientButtonClassName =
  "bg-gradient-to-r from-primary via-fuchsia-500 to-violet-600 text-white shadow-[0_14px_34px_rgba(168,85,247,0.28)] hover:brightness-110 hover:saturate-125";

export function PpvFavoritesPage() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [aiQuery, setAiQuery] = useState("");
  const [aiSubmittedQuery, setAiSubmittedQuery] = useState("");
  const [aiFavoriteOverrides, setAiFavoriteOverrides] = useState<Record<string, boolean>>(
    {},
  );
  const filterPpvByDate = useChannelSettingsStore(
    (state) => state.filterPpvByDate,
  );
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
  const aiSearchMutation = useMutation({
    mutationFn: searchAiPpv,
    onSuccess: () => {
      setAiFavoriteOverrides({});
    },
  });

  const fetchedFavorites = favoritesQuery.data ?? [];
  const favoriteReferenceNow = new Date(Date.now());
  const {
    liveFavorites,
    displayedFavorites: favorites,
  } = splitPpvFavorites(fetchedFavorites, favoriteReferenceNow);

  function persistFavoriteOrder(nextFavorites: FavoriteChannelEntry[]) {
    reorderMutation.mutate({
      channelIds: nextFavorites.map((entry) => entry.channel.id),
    });
  }

  function moveChannel(index: number, direction: -1 | 1) {
    const currentEntry = favorites[index];
    const nextEntry = favorites[index + direction];

    if (
      !currentEntry
      || !nextEntry
      || isLiveFavorite(currentEntry, favoriteReferenceNow)
        !== isLiveFavorite(nextEntry, favoriteReferenceNow)
    ) {
      return;
    }

    const nextChannels = moveEntry(favorites, index, direction);
    if (nextChannels === favorites) {
      return;
    }
    persistFavoriteOrder(nextChannels);
  }

  function submitAiSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = aiQuery.trim();
    if (query.length < 3) {
      return;
    }
    setAiSubmittedQuery(query);
    aiSearchMutation.mutate({ query, limit: 12 });
  }

  function toggleAiPpvFavorite(result: AiPpvSearchResult) {
    const channelId = result.channel.id;
    const previousIsPpvFavorite = Boolean(result.channel.isPpvFavorite);
    setAiFavoriteOverrides((current) => ({
      ...current,
      [channelId]: !previousIsPpvFavorite,
    }));
    favoriteMutation.mutate(result.channel, {
      onError: () => {
        setAiFavoriteOverrides((current) => ({
          ...current,
          [channelId]: previousIsPpvFavorite,
        }));
      },
    });
  }

  const aiResults = (aiSearchMutation.data?.items ?? [])
    .filter((result) => shouldShowChannelForPpvDateFilter(result.channel, {
      enabled: filterPpvByDate,
    }))
    .map((result) => {
      const override = aiFavoriteOverrides[result.channel.id];
      if (override === undefined) {
        return result;
      }

      return {
        ...result,
        channel: {
          ...result.channel,
          isPpvFavorite: override,
        },
      };
    });

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

      <AiPpvSearchPanel
        query={aiQuery}
        submittedQuery={aiSubmittedQuery}
        results={aiResults}
        backend={aiSearchMutation.data?.backend}
        message={aiSearchMutation.data?.message}
        isPending={aiSearchMutation.isPending}
        isError={aiSearchMutation.isError}
        error={aiSearchMutation.error}
        playPending={playMutation.isPending}
        favoritePending={favoriteMutation.isPending}
        favoritePendingChannelId={favoriteMutation.variables?.id}
        onQueryChange={setAiQuery}
        onSubmit={submitAiSearch}
        onPlay={(channelId) => playMutation.mutate(channelId)}
        onTogglePpvFavorite={toggleAiPpvFavorite}
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
        <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:border-primary/20 sm:bg-gradient-to-br sm:from-card sm:via-card sm:to-primary/10 sm:shadow-[0_18px_70px_rgba(0,0,0,0.22)] sm:ring-1 sm:ring-primary/10">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-5 sm:pb-0">
            <div className="flex flex-wrap items-center gap-2">
              <CardTitle>Saved PPV events</CardTitle>
              {liveFavorites.length ? (
                <Badge variant="accent" className={ppvLiveBadgeClassName}>
                  <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
                  {liveFavorites.length} live now
                </Badge>
              ) : null}
            </div>
          </CardHeader>
          <CardContent className="p-0">
            {favorites.map((entry, index) => {
              const { channel, program } = entry;
              const displayChannelName = formatEventChannelTitle(channel.name, {
                referenceStartAt: program?.startAt,
              });
              const liveNow = isLiveFavorite(entry, favoriteReferenceNow);
              const previousLiveState = index > 0
                ? isLiveFavorite(favorites[index - 1], favoriteReferenceNow)
                : null;
              const startsLiveSection = liveNow && index === 0;
              const startsSavedSection = !liveNow && liveFavorites.length > 0 && previousLiveState === true;
              const canMoveUp = index > 0 && liveNow === previousLiveState;
              const canMoveDown = index < favorites.length - 1
                && liveNow === isLiveFavorite(favorites[index + 1], favoriteReferenceNow);

              return (
                <div key={channel.id} className="group">
                  {startsLiveSection ? (
                    <div className="flex items-center gap-3 px-4 pb-2 pt-1 text-xs font-medium text-primary sm:px-5">
                      <span
                        className="size-2 rounded-full bg-gradient-to-br from-fuchsia-200 via-primary to-violet-700 shadow-[0_0_18px_rgba(168,85,247,0.8)]"
                        aria-hidden="true"
                      />
                      <span>Live now</span>
                      <div className="h-px flex-1 bg-gradient-to-r from-primary/45 via-primary/20 to-transparent" />
                    </div>
                  ) : null}
                  {startsSavedSection ? (
                    <div className="flex items-center gap-3 px-4 pb-2 pt-3 text-xs font-medium text-muted-foreground sm:px-5">
                      <span>Saved for later</span>
                      <div className="h-px flex-1 bg-border/70" />
                    </div>
                  ) : null}
                  {index > 0 && !startsSavedSection ? <Separator /> : null}
                  <div
                    className={cn(
                      "relative flex flex-col gap-4 p-4 transition-colors hover:bg-muted/30 sm:gap-5 sm:p-5",
                      liveNow && "bg-gradient-to-r from-primary/15 via-primary/5 to-transparent ring-1 ring-inset ring-primary/10 hover:from-primary/20 hover:via-primary/10",
                    )}
                  >
                    {liveNow ? (
                      <div
                        aria-hidden="true"
                        className="absolute bottom-3 left-0 top-3 w-1 rounded-r-full bg-gradient-to-b from-fuchsia-200 via-primary to-violet-700 shadow-[0_0_24px_rgba(168,85,247,0.55)]"
                      />
                    ) : null}
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
                            {liveNow ? (
                              <Badge variant="accent" className={ppvLiveBadgeClassName}>
                                <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
                                Live now
                              </Badge>
                            ) : null}
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
                              <Badge variant="accent" className="h-5 border-transparent bg-gradient-to-r from-primary/20 to-violet-500/10 px-1.5 py-0 text-[10px] font-medium tracking-wide text-primary ring-primary/25">
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
                          canMoveUp={canMoveUp}
                          canMoveDown={canMoveDown}
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
                          className={cn("min-w-24", ppvGradientButtonClassName)}
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
                        canMoveUp={canMoveUp}
                        canMoveDown={canMoveDown}
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
                        className={cn("flex-1", ppvGradientButtonClassName)}
                        onClick={() => playMutation.mutate(channel.id)}
                        disabled={playMutation.isPending}
                      >
                        <Play data-icon="inline-start" />
                        Play
                      </Button>
                    </div>
                    {program ? (
                      <div
                        className={cn(
                          "rounded-xl border border-border/40 bg-secondary/20 p-3.5 sm:p-4",
                          liveNow && "border-primary/20 bg-gradient-to-br from-primary/10 via-primary/5 to-violet-500/10 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]",
                        )}
                      >
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

function AiPpvSearchPanel({
  query,
  submittedQuery,
  results,
  backend,
  message,
  isPending,
  isError,
  error,
  playPending,
  favoritePending,
  favoritePendingChannelId,
  onQueryChange,
  onSubmit,
  onPlay,
  onTogglePpvFavorite,
}: {
  query: string;
  submittedQuery: string;
  results: AiPpvSearchResult[];
  backend?: "openrouter" | "local_fallback";
  message?: string | null;
  isPending: boolean;
  isError: boolean;
  error: Error | null;
  playPending: boolean;
  favoritePending: boolean;
  favoritePendingChannelId?: string;
  onQueryChange: (query: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onPlay: (channelId: string) => void;
  onTogglePpvFavorite: (result: AiPpvSearchResult) => void;
}) {
  const canSearch = query.trim().length >= 3 && !isPending;
  const hasSearched = Boolean(submittedQuery);

  return (
    <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:border-primary/20 sm:bg-gradient-to-br sm:from-card sm:via-card sm:to-primary/10 sm:shadow-sm">
      <CardHeader className="px-0 pb-3 pt-0 sm:p-5 sm:pb-0">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <Sparkles className="size-4" aria-hidden="true" />
            </div>
            <CardTitle>AI PPV search</CardTitle>
          </div>
          {backend ? (
            <Badge variant={backend === "openrouter" ? "accent" : "outline"}>
              {backend === "openrouter" ? "OpenRouter" : "Local fallback"}
            </Badge>
          ) : null}
        </div>
      </CardHeader>
      <CardContent className="space-y-4 px-0 pb-0 pt-0 sm:p-5 sm:pt-4">
        <form className="grid gap-3" onSubmit={onSubmit}>
          <label className="sr-only" htmlFor="ai-ppv-search">
            Describe a PPV event
          </label>
          <textarea
            id="ai-ppv-search"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Describe an event, team matchup, tournament, or provider clue"
            className="min-h-24 w-full resize-y rounded-xl border border-input bg-background px-3 py-3 text-sm shadow-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          />
          <div className="flex flex-wrap items-center gap-2">
            <Button type="submit" disabled={!canSearch}>
              <Search data-icon="inline-start" />
              {isPending ? "Searching..." : "Find PPV matches"}
            </Button>
            {message ? (
              <span className="text-sm text-muted-foreground">{message}</span>
            ) : null}
            {isError ? (
              <span className="text-sm text-destructive">
                {error?.message || "AI PPV search failed"}
              </span>
            ) : null}
          </div>
        </form>

        {isPending ? (
          <div className="grid gap-3">
            {Array.from({ length: 3 }).map((_, index) => (
              <div
                key={index}
                className="flex items-center gap-4 rounded-xl border border-border/70 p-4"
              >
                <Skeleton className="size-11 rounded-2xl" />
                <div className="flex flex-1 flex-col gap-2">
                  <Skeleton className="h-4 w-48" />
                  <Skeleton className="h-4 w-72 max-w-full" />
                </div>
                <Skeleton className="h-9 w-24 rounded-lg" />
              </div>
            ))}
          </div>
        ) : null}

        {!isPending && hasSearched && !results.length && !isError ? (
          <Empty className="rounded-xl border border-border/60 bg-background/45">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Search aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>No AI PPV matches</EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : null}

        {!isPending && results.length ? (
          <div className="overflow-hidden rounded-xl border border-border/70 bg-background/45">
            {results.map((result, index) => (
              <div key={result.channel.id}>
                {index > 0 ? <Separator /> : null}
                <AiPpvSearchResultRow
                  result={result}
                  playPending={playPending}
                  favoritePending={
                    favoritePending && favoritePendingChannelId === result.channel.id
                  }
                  onPlay={onPlay}
                  onTogglePpvFavorite={onTogglePpvFavorite}
                />
              </div>
            ))}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function AiPpvSearchResultRow({
  result,
  playPending,
  favoritePending,
  onPlay,
  onTogglePpvFavorite,
}: {
  result: AiPpvSearchResult;
  playPending: boolean;
  favoritePending: boolean;
  onPlay: (channelId: string) => void;
  onTogglePpvFavorite: (result: AiPpvSearchResult) => void;
}) {
  const { channel, program } = result;
  const displayChannelName = formatEventChannelTitle(channel.name, {
    referenceStartAt: program?.startAt,
  });
  const confidence = Math.round(result.confidence * 100);

  return (
    <div className="flex flex-col gap-4 p-4 sm:p-5 lg:flex-row lg:items-start lg:justify-between">
      <div className="flex min-w-0 flex-1 items-start gap-3 sm:gap-4">
        <ChannelAvatar
          name={displayChannelName}
          logoUrl={channel.logoUrl}
          className="size-12 shrink-0 rounded-xl sm:size-14 sm:rounded-2xl"
          fallbackClassName="rounded-xl sm:rounded-2xl"
        />
        <div className="min-w-0 flex-1 space-y-2">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <h2 className="min-w-0 break-words text-base font-semibold tracking-tight">
              {displayChannelName}
            </h2>
            <Badge variant="accent">PPV</Badge>
            <Badge variant="outline">{confidence}% match</Badge>
            {channel.categoryName ? (
              <Badge variant="outline">{channel.categoryName}</Badge>
            ) : null}
            {channel.isPpvFavorite ? <Badge variant="outline">PPV saved</Badge> : null}
          </div>
          <p className="text-sm text-muted-foreground">{result.reason}</p>
          {result.matchedTerms.length ? (
            <div className="flex flex-wrap gap-1.5">
              {result.matchedTerms.map((term) => (
                <Badge key={term} variant="outline" className="h-5 px-1.5 py-0 text-[11px]">
                  {term}
                </Badge>
              ))}
            </div>
          ) : null}
          {program ? (
            <div className="rounded-xl border border-border/50 bg-card/60 p-3">
              <FavoriteProgramDetails program={program} />
            </div>
          ) : null}
        </div>
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2 lg:pt-1">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => onTogglePpvFavorite(result)}
          disabled={favoritePending}
        >
          <Heart data-icon="inline-start" />
          {channel.isPpvFavorite ? "Remove PPV" : "Save PPV"}
        </Button>
        <Button
          size="sm"
          className={ppvGradientButtonClassName}
          onClick={() => onPlay(channel.id)}
          disabled={playPending}
        >
          <Play data-icon="inline-start" />
          Play
        </Button>
      </div>
    </div>
  );
}
