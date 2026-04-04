import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, ChevronDown, ChevronRight, Heart, Play, Radio, RefreshCcw, Search as SearchIcon, SlidersHorizontal, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { KeyboardEvent } from "react";
import type { GuideCategorySummary, GuidePreferences } from "@euripus/shared";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useTvAutoFocus } from "@/hooks/use-tv-auto-focus";
import { getGuide, getGuideCategory, getGuidePreferences, saveGuidePreferences, startChannelPlayback } from "@/lib/api";
import { cn, formatArchiveDuration, formatTimeRange, getTimeProgress } from "@/lib/utils";
import { usePlayerStore } from "@/store/player-store";

const GUIDE_PAGE_SIZE = 40;

type GuideCategorySectionProps = {
  category: GuideCategorySummary;
  open: boolean;
  favoritePending: boolean;
  activeFavoriteChannelId?: string;
  onToggle: (nextOpen: boolean) => void;
  onFavorite: (channel: {
    id: string;
    name: string;
    logoUrl: string | null;
    categoryName: string | null;
    remoteStreamId: number;
    epgChannelId: string | null;
    hasCatchup: boolean;
    archiveDurationHours: number | null;
    streamExtension: string | null;
    isFavorite: boolean;
  }) => void;
  onPlay: (channelId: string) => void;
};

type GuideCategoryFilterCardProps = {
  categories: GuideCategorySummary[];
  filterInput: string;
  appliedFilter: string;
  preferencesReady: boolean;
  saving: boolean;
  selectedCategoryIds: string[];
  onFilterInputChange: (value: string) => void;
  onApplyFilter: () => void;
  onReset: () => void;
  onToggleCategory: (categoryId: string) => void;
};

export function GuidePage() {
  const queryClient = useQueryClient();
  const [openCategories, setOpenCategories] = useState<string[]>([]);
  const [filterInput, setFilterInput] = useState("");
  const [appliedFilter, setAppliedFilter] = useState("");
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const guideQuery = useQuery({
    queryKey: ["guide", "overview"],
    queryFn: getGuide,
  });
  const preferencesQuery = useQuery({
    queryKey: ["guide", "preferences"],
    queryFn: getGuidePreferences,
  });
  const savePreferencesMutation = useMutation({
    mutationFn: saveGuidePreferences,
    onMutate: async (nextPreferences) => {
      await queryClient.cancelQueries({ queryKey: ["guide", "preferences"] });
      const previousPreferences = queryClient.getQueryData<GuidePreferences>(["guide", "preferences"]);
      queryClient.setQueryData<GuidePreferences>(["guide", "preferences"], nextPreferences);
      return { previousPreferences };
    },
    onError: (_error, _variables, context) => {
      if (context?.previousPreferences) {
        queryClient.setQueryData(["guide", "preferences"], context.previousPreferences);
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: ["guide", "preferences"] });
    },
  });
  const favoriteMutation = useChannelFavoriteMutation();
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
  const availableCategoryIds = useMemo(() => new Set(categories.map((category) => category.id)), [categories]);
  const savedCategoryIds = preferencesQuery.data?.includedCategoryIds ?? [];
  const validSelectedCategoryIds = useMemo(
    () => savedCategoryIds.filter((categoryId) => availableCategoryIds.has(categoryId)),
    [availableCategoryIds, savedCategoryIds],
  );
  const selectedCategoryIdsSet = useMemo(() => new Set(validSelectedCategoryIds), [validSelectedCategoryIds]);
  const normalizedAppliedFilter = appliedFilter.trim().toLowerCase();
  const shouldApplyFilter = normalizedAppliedFilter.length >= 2;
  const chooserCategories = useMemo(() => {
    if (!shouldApplyFilter) {
      return categories;
    }

    return categories.filter((category) => category.name.toLowerCase().includes(normalizedAppliedFilter));
  }, [categories, normalizedAppliedFilter, shouldApplyFilter]);
  const selectedCategories = useMemo(() => {
    if (!validSelectedCategoryIds.length) {
      return categories;
    }

    return categories.filter((category) => selectedCategoryIdsSet.has(category.id));
  }, [categories, selectedCategoryIdsSet, validSelectedCategoryIds.length]);
  const visibleCategories = useMemo(() => {
    if (!shouldApplyFilter) {
      return selectedCategories;
    }

    return selectedCategories.filter((category) => category.name.toLowerCase().includes(normalizedAppliedFilter));
  }, [normalizedAppliedFilter, selectedCategories, shouldApplyFilter]);
  useTvAutoFocus(
    visibleCategories.length ? "[data-guide-category-filter='true']" : "[data-guide-category-toggle='true']",
    [visibleCategories.length, openCategories.join("|")],
  );

  useEffect(() => {
    if (!preferencesQuery.data || !categories.length || savePreferencesMutation.isPending) {
      return;
    }

    if (!areCategoryIdsEqual(savedCategoryIds, validSelectedCategoryIds)) {
      savePreferencesMutation.mutate({ includedCategoryIds: validSelectedCategoryIds });
    }
  }, [
    categories.length,
    preferencesQuery.data,
    savePreferencesMutation,
    savePreferencesMutation.isPending,
    savedCategoryIds,
    validSelectedCategoryIds,
  ]);

  function updateIncludedCategoryIds(includedCategoryIds: string[]) {
    savePreferencesMutation.mutate({ includedCategoryIds });
  }

  function applyFilter() {
    const nextFilter = filterInput.trim();
    setAppliedFilter(nextFilter.length >= 2 ? nextFilter : "");
  }

  function toggleIncludedCategory(categoryId: string) {
    if (selectedCategoryIdsSet.has(categoryId)) {
      updateIncludedCategoryIds(validSelectedCategoryIds.filter((id) => id !== categoryId));
      return;
    }

    updateIncludedCategoryIds([...validSelectedCategoryIds, categoryId]);
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Live Guide"
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
            <Badge variant="outline">
              {shouldApplyFilter
                ? `${visibleCategories.length} matching`
                : validSelectedCategoryIds.length
                  ? `${validSelectedCategoryIds.length} included`
                  : "Showing all"}
            </Badge>
          </>
        }
      />

      {!guideQuery.isPending && categories.length ? (
        <GuideCategoryFilterCard
          categories={chooserCategories}
          filterInput={filterInput}
          appliedFilter={appliedFilter}
          preferencesReady={!preferencesQuery.isPending}
          saving={savePreferencesMutation.isPending}
          selectedCategoryIds={validSelectedCategoryIds}
          onFilterInputChange={setFilterInput}
          onApplyFilter={applyFilter}
          onReset={() => updateIncludedCategoryIds([])}
          onToggleCategory={toggleIncludedCategory}
        />
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
                    favoritePending={favoriteMutation.isPending}
                    activeFavoriteChannelId={favoriteMutation.variables?.id}
                    onToggle={(nextOpen) => toggleCategory(category.id, nextOpen)}
                    onFavorite={(channel) => favoriteMutation.mutate(channel)}
                    onPlay={(channelId) => playMutation.mutate(channelId)}
                  />
                </div>
              ))
            ) : (
              <Empty className="border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <SlidersHorizontal aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyTitle>{shouldApplyFilter ? "No categories match this filter" : "No categories selected"}</EmptyTitle>
                </EmptyHeader>
              </Empty>
            )}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}

function GuideCategoryFilterCard({
  categories,
  filterInput,
  appliedFilter,
  preferencesReady,
  saving,
  selectedCategoryIds,
  onFilterInputChange,
  onApplyFilter,
  onReset,
  onToggleCategory,
}: GuideCategoryFilterCardProps) {
  const [open, setOpen] = useState(true);
  const selectedCategoryIdSet = useMemo(() => new Set(selectedCategoryIds), [selectedCategoryIds]);
  const ToggleIcon = open ? ChevronDown : ChevronRight;
  const showAppliedFilter = appliedFilter.trim().length >= 2;

  function handleFilterKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") {
      return;
    }

    event.preventDefault();
    onApplyFilter();
  }

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <Card className="overflow-hidden border-border/80 bg-gradient-to-r from-card via-card to-primary/5">
        <CardHeader className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-2xl bg-primary/10 text-primary">
              <SlidersHorizontal aria-hidden="true" />
            </div>
            <div className="flex flex-col gap-1">
              <CardTitle>Included categories</CardTitle>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={selectedCategoryIds.length ? "accent" : "outline"}>
              {selectedCategoryIds.length ? `${selectedCategoryIds.length} selected` : "All categories"}
            </Badge>
            {showAppliedFilter ? <Badge variant="outline">Filter: {appliedFilter.trim()}</Badge> : null}
            {open ? (
              <Button variant="ghost" size="sm" onClick={onReset} disabled={!selectedCategoryIds.length || saving}>
                <X data-icon="inline-start" />
                Show all
              </Button>
            ) : null}
            <CollapsibleTrigger asChild>
              <Button variant="ghost" size="sm" aria-expanded={open}>
                <ToggleIcon data-icon="inline-start" />
                {open ? "Hide filter" : "Show filter"}
              </Button>
            </CollapsibleTrigger>
          </div>
        </CardHeader>

        <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          <CardContent className="flex flex-col gap-4">
            <div className="relative">
              <SearchIcon className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
              <Input
                className="pl-10"
                placeholder="Type 2+ characters, then press Enter..."
                value={filterInput}
                onChange={(event) => onFilterInputChange(event.target.value)}
                onKeyDown={handleFilterKeyDown}
                disabled={!preferencesReady}
              />
            </div>

            <ScrollArea
              type="always"
              className="h-[28rem] rounded-2xl border border-border/70 bg-background/70"
              data-testid="guide-category-filter-scroll-area"
            >
              <div className="flex flex-col p-2 pr-3 [content-visibility:auto]">
                {categories.length ? (
                  categories.map((category) => {
                    const selected = selectedCategoryIdSet.has(category.id);

                    return (
                      <button
                        key={category.id}
                        type="button"
                        data-tv-focusable="true"
                        data-guide-category-filter="true"
                        data-tv-autofocus={selected ? "true" : undefined}
                        aria-pressed={selected}
                        disabled={!preferencesReady || saving}
                        onClick={() => onToggleCategory(category.id)}
                        className={cn(
                          "flex items-center justify-between gap-3 rounded-xl px-3 py-3 text-left transition-colors",
                          selected ? "bg-primary/10 text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground",
                        )}
                      >
                        <div className="flex min-w-0 items-center gap-3">
                          <div
                            className={cn(
                              "flex size-6 shrink-0 items-center justify-center rounded-full border",
                              selected ? "border-primary bg-primary text-primary-foreground" : "border-border bg-background",
                            )}
                          >
                            {selected ? <Check className="size-3.5" aria-hidden="true" /> : null}
                          </div>
                          <span className="truncate font-medium">{category.name}</span>
                        </div>
                        <div className="flex shrink-0 items-center gap-2">
                          <Badge variant="outline">{category.channelCount}</Badge>
                          <Badge variant={category.liveNowCount ? "live" : "outline"}>{category.liveNowCount}</Badge>
                        </div>
                      </button>
                    );
                  })
                ) : (
                  <div className="px-4 py-8 text-center text-sm text-muted-foreground">
                    {showAppliedFilter ? "No matching categories" : "Type 2+ characters and press Enter to apply a custom filter"}
                  </div>
                )}
              </div>
            </ScrollArea>
          </CardContent>
        </CollapsibleContent>
      </Card>
    </Collapsible>
  );
}

function GuideCategorySection({
  category,
  open,
  favoritePending,
  activeFavoriteChannelId,
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
          <div className="flex min-w-0 flex-1 flex-col gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="text-lg font-semibold">{category.name}</h2>
              <Badge variant="outline">{category.channelCount} channels</Badge>
              <Badge variant={category.liveNowCount ? "live" : "outline"}>{category.liveNowCount} live now</Badge>
            </div>
          </div>
          <CollapsibleTrigger asChild>
            <Button variant="ghost" aria-expanded={open} data-guide-category-toggle="true" data-tv-autofocus={open ? "true" : undefined}>
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

          {categoryQuery.isError ? <div className="p-5 text-sm text-destructive">Unable to load this category right now.</div> : null}

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
                            ) : null}
                          </div>
                        </div>
                      </div>

                      <div className="flex flex-wrap items-center gap-2">
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={() => onFavorite(channel)}
                          disabled={favoritePending && activeFavoriteChannelId === channel.id}
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
              </EmptyHeader>
            </Empty>
          ) : null}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

function areCategoryIdsEqual(current: string[], next: string[]) {
  if (current.length !== next.length) {
    return false;
  }

  return current.every((value, index) => value === next[index]);
}

function isProgramLive(startAt: string, endAt: string) {
  const now = Date.now();
  return new Date(startAt).getTime() <= now && new Date(endAt).getTime() > now;
}
