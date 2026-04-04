import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Heart, Play, Radio, RefreshCcw, Search as SearchIcon } from "lucide-react";
import { useMemo, useState } from "react";
import type { GuideCategorySummary } from "@euripus/shared";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { addFavorite, getGuide, getGuideCategory, removeFavorite, startChannelPlayback } from "@/lib/api";
import { formatArchiveDuration, formatTimeRange, getTimeProgress } from "@/lib/utils";
import { usePlayerStore } from "@/store/player-store";

const GUIDE_PAGE_SIZE = 40;

type FavoriteMutationPayload = {
  channelId: string;
  favorite: boolean;
};

type GuideCategorySectionProps = {
  category: GuideCategorySummary;
  open: boolean;
  onToggle: (nextOpen: boolean) => void;
  onFavorite: (payload: FavoriteMutationPayload) => void;
  onPlay: (channelId: string) => void;
};

export function GuidePage() {
  const queryClient = useQueryClient();
  const [openCategories, setOpenCategories] = useState<string[]>([]);
  const [filterQuery, setFilterQuery] = useState("");
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const guideQuery = useQuery({
    queryKey: ["guide", "overview"],
    queryFn: getGuide,
  });
  const favoriteMutation = useMutation({
    mutationFn: async ({ channelId, favorite }: FavoriteMutationPayload) =>
      favorite ? removeFavorite(channelId) : addFavorite(channelId),
    onMutate: async ({ channelId, favorite }) => {
      await queryClient.cancelQueries({ queryKey: ["guide"] });
      await queryClient.cancelQueries({ queryKey: ["favorites"] });

      queryClient.setQueriesData<{ pages: Array<{ entries: Array<{ channel: { id: string; isFavorite: boolean } }> }> }>(
        { queryKey: ["guide", "category"] },
        (old) => {
          if (!old) return old;
          return {
            ...old,
            pages: old.pages.map((page) => ({
              ...page,
              entries: page.entries.map((entry) =>
                entry.channel.id === channelId
                  ? { ...entry, channel: { ...entry.channel, isFavorite: !favorite } }
                  : entry,
              ),
            })),
          };
        },
      );
    },
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
        queryClient.invalidateQueries({ queryKey: ["guide", "overview"] }),
        queryClient.invalidateQueries({ queryKey: ["guide", "category"] }),
      ]);
    },
  });
  const playMutation = useMutation({
    mutationFn: startChannelPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });

  function toggleCategory(categoryId: string, nextOpen: boolean) {
    setOpenCategories((current) =>
      nextOpen ? [...new Set([...current, categoryId])] : current.filter((id) => id !== categoryId),
    );
  }

  async function refreshGuide() {
    await queryClient.invalidateQueries({ queryKey: ["guide"] });
  }

  const categories = guideQuery.data?.categories ?? [];
  const liveCount = categories.reduce((sum, category) => sum + category.liveNowCount, 0);
  const normalizedFilter = filterQuery.trim().toLowerCase();
  const visibleCategories = useMemo(() => {
    if (!normalizedFilter) {
      return categories;
    }

    return categories.filter((category) => category.name.toLowerCase().includes(normalizedFilter));
  }, [categories, normalizedFilter]);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Live Guide"
        description="Browse live categories, see what is on right now, and jump straight into playback from a cleaner guide surface."
        actions={
          <Button variant="outline" onClick={refreshGuide}>
            <RefreshCcw data-icon="inline-start" />
            Refresh
          </Button>
        }
        meta={
          <>
            <Badge variant="accent">{categories.length} categories</Badge>
            <Badge variant="outline">{liveCount} live now</Badge>
          </>
        }
      />

      {!guideQuery.isPending ? (
        <div className="relative max-w-md">
          <SearchIcon className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
          <Input
            className="pl-10"
            placeholder="Filter guide categories..."
            value={filterQuery}
            onChange={(event) => setFilterQuery(event.target.value)}
          />
        </div>
      ) : null}

      {guideQuery.isPending ? (
        <Card>
          <CardContent className="flex flex-col gap-3 pt-5">
            {Array.from({ length: 5 }).map((_, index) => (
              <div key={index} className="rounded-2xl border border-border/70 p-5">
                <Skeleton className="h-5 w-40" />
                <Skeleton className="mt-3 h-4 w-24" />
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}

      {!guideQuery.isPending && !categories.length ? (
        <Card>
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Radio aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>No guide data yet</EmptyTitle>
                <EmptyDescription>Connect a provider and run a sync to populate channels and live guide results.</EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!guideQuery.isPending && categories.length ? (
        <Card>
          <CardContent className="p-0">
            {visibleCategories.length ? (
              visibleCategories.map((category, index) => (
                <div key={category.id}>
                  {index > 0 ? <Separator /> : null}
                  <GuideCategorySection
                    category={category}
                    open={openCategories.includes(category.id)}
                    onToggle={(nextOpen) => toggleCategory(category.id, nextOpen)}
                    onFavorite={(payload) => favoriteMutation.mutate(payload)}
                    onPlay={(channelId) => playMutation.mutate(channelId)}
                  />
                </div>
              ))
            ) : (
              <Empty className="border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <SearchIcon aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyTitle>No guide matches</EmptyTitle>
                  <EmptyDescription>Try a broader guide category name.</EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}

function GuideCategorySection({
  category,
  open,
  onToggle,
  onFavorite,
  onPlay,
}: GuideCategorySectionProps) {
  const categoryQuery = useInfiniteQuery({
    queryKey: ["guide", "category", category.id],
    queryFn: ({ pageParam }) => getGuideCategory(category.id, pageParam, GUIDE_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: open,
  });
  const entries = categoryQuery.data?.pages.flatMap((page) => page.entries) ?? [];
  const hasEntries = entries.length > 0;
  const isInitialLoading = open && categoryQuery.isLoading && !hasEntries;
  const Icon = open ? ChevronDown : ChevronRight;

  return (
    <Collapsible open={open} onOpenChange={onToggle}>
      <div className="flex flex-col gap-4 p-5">
        <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="text-lg font-semibold">{category.name}</h2>
              <Badge variant="outline">{category.channelCount} channels</Badge>
              <Badge variant={category.liveNowCount ? "live" : "outline"}>{category.liveNowCount} live now</Badge>
            </div>
            <p className="text-sm text-muted-foreground">
              {category.channelCount === 1 ? "1 channel in this guide category." : `${category.channelCount} channels in this guide category.`}
            </p>
          </div>
          <CollapsibleTrigger asChild>
            <Button variant="ghost" aria-expanded={open}>
              <Icon data-icon="inline-start" />
              {open ? "Hide channels" : "Show channels"}
            </Button>
          </CollapsibleTrigger>
        </div>
      </div>

      <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
        <Separator />
        <div className="flex flex-col">
          {isInitialLoading ? (
            <div className="flex flex-col gap-3 p-5">
              {Array.from({ length: 3 }).map((_, index) => (
                <div key={index} className="flex items-center gap-4 rounded-2xl border border-border/70 p-4">
                  <Skeleton className="size-11 rounded-2xl" />
                  <div className="flex flex-1 flex-col gap-2">
                    <Skeleton className="h-5 w-40" />
                    <Skeleton className="h-4 w-64" />
                  </div>
                </div>
              ))}
            </div>
          ) : null}

            {categoryQuery.isError ? (
              <div className="p-5 text-sm text-destructive">Unable to load this category right now.</div>
            ) : null}

            {hasEntries
              ? entries.map(({ channel, program }, index) => {
                  const programIsLive = program ? isProgramLive(program.startAt, program.endAt) : false;

                  return (
                    <div key={channel.id}>
                      {index > 0 ? <Separator /> : null}
                      <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                        <div className="flex min-w-0 items-start gap-4">
                          <ChannelAvatar name={channel.name} logoUrl={channel.logoUrl} />
                          <div className="flex min-w-0 flex-1 flex-col gap-3">
                            <div className="flex flex-wrap items-center gap-2">
                              <h3 className="truncate text-base font-semibold">{channel.name}</h3>
                              {channel.hasCatchup ? <Badge variant="live">Catch-up</Badge> : null}
                              {channel.archiveDurationHours ? (
                                <Badge variant="outline">{formatArchiveDuration(channel.archiveDurationHours)}</Badge>
                              ) : null}
                              {program && programIsLive ? <Badge variant="accent">Live now</Badge> : null}
                            </div>

                            <div className="flex min-w-0 flex-col gap-2">
                              <p className="text-sm font-medium">
                                {program?.title ?? "No current program metadata synced yet."}
                              </p>
                              {program ? (
                                <>
                                  <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                                    <span>{formatTimeRange(program.startAt, program.endAt)}</span>
                                    {program.canCatchup ? <Badge variant="outline">Catch-up window</Badge> : null}
                                  </div>
                                  {programIsLive ? <Progress value={getTimeProgress(program.startAt, program.endAt)} className="h-2" /> : null}
                                </>
                              ) : (
                                <p className="text-sm text-muted-foreground">
                                  Run a fresh guide sync if this channel should have live EPG data.
                                </p>
                              )}
                            </div>
                          </div>
                        </div>

                        <div className="flex flex-wrap items-center gap-2">
                          <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => onFavorite({ channelId: channel.id, favorite: channel.isFavorite })}
                          >
                            <Heart data-icon="inline-start" />
                            {channel.isFavorite ? "Unfavorite" : "Favorite"}
                          </Button>
                          <Button size="sm" onClick={() => onPlay(channel.id)}>
                            <Play data-icon="inline-start" />
                            Play
                          </Button>
                        </div>
                      </div>
                    </div>
                  );
                })
              : null}

            {categoryQuery.hasNextPage ? (
              <>
                <Separator />
                <div className="p-5">
                  <Button
                    variant="outline"
                    onClick={() => categoryQuery.fetchNextPage()}
                    disabled={categoryQuery.isFetchingNextPage}
                  >
                    {categoryQuery.isFetchingNextPage ? "Loading more..." : "Load more"}
                  </Button>
                </div>
              </>
            ) : null}

            {!isInitialLoading && open && !hasEntries && !categoryQuery.isError ? (
              <Empty className="border-0">
                <EmptyHeader>
                  <EmptyTitle>No channels available in this category</EmptyTitle>
                  <EmptyDescription>No channels with matching EPG data are available in this category right now.</EmptyDescription>
                </EmptyHeader>
              </Empty>
            ) : null}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

function isProgramLive(startAt: string, endAt: string) {
  const now = Date.now();
  return new Date(startAt).getTime() <= now && new Date(endAt).getTime() > now;
}
