import { useInfiniteQuery } from "@tanstack/react-query";
import type { Channel, GuideCategorySummary } from "@euripus/shared";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Heart,
  Play,
  Search as SearchIcon,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { KeyboardEvent } from "react";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Empty,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Toggle } from "@/components/ui/toggle";
import { getGuideCategory } from "@/lib/api";
import {
  cn,
  formatArchiveDuration,
  formatTimeRange,
  getTimeProgress,
} from "@/lib/utils";

const GUIDE_PAGE_SIZE = 40;

type GuideCategoryFilterCardProps = {
  categories: GuideCategorySummary[];
  filterInput: string;
  appliedFilter: string;
  preferencesReady: boolean;
  saving: boolean;
  selectedCategoryIds: string[];
  showOnlyChannelsWithEpg: boolean;
  onFilterInputChange: (value: string) => void;
  onApplyFilter: () => void;
  onReset: () => void;
  onToggleEpgOnly: (pressed: boolean) => void;
  onToggleCategory: (categoryId: string) => void;
};

type GuideCategorySectionProps = {
  category: GuideCategorySummary;
  open: boolean;
  favoritePending: boolean;
  activeFavoriteChannelId?: string;
  categoryFavoritePending: boolean;
  activeFavoriteCategoryId?: string;
  showOnlyChannelsWithEpg: boolean;
  onToggle: (nextOpen: boolean) => void;
  onToggleCategoryFavorite: (category: GuideCategorySummary) => void;
  onFavorite: (channel: Channel) => void;
  onPlay: (channelId: string) => void;
};

export function GuideCategoryFilterCard({
  categories,
  filterInput,
  appliedFilter,
  preferencesReady,
  saving,
  selectedCategoryIds,
  showOnlyChannelsWithEpg,
  onFilterInputChange,
  onApplyFilter,
  onReset,
  onToggleEpgOnly,
  onToggleCategory,
}: GuideCategoryFilterCardProps) {
  const [open, setOpen] = useState(false);
  const selectedCategoryIdSet = useMemo(
    () => new Set(selectedCategoryIds),
    [selectedCategoryIds],
  );
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
      <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:border-border/80 sm:bg-gradient-to-r sm:from-card sm:via-card sm:to-primary/5 sm:shadow-sm">
        <CardHeader className="px-0 pt-0 pb-4 sm:p-5 sm:pb-0">
          <div className="flex flex-col gap-4 sm:grid sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-x-4 sm:gap-y-3">
            <div className="flex min-h-10 min-w-0 items-center gap-3">
              <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-primary">
                <SlidersHorizontal aria-hidden="true" />
              </div>
              <div className="flex h-10 min-w-0 items-center">
                <CardTitle className="min-w-0 leading-none">
                  Included categories
                </CardTitle>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2 sm:justify-self-end">
              <Badge
                variant={selectedCategoryIds.length ? "accent" : "outline"}
              >
                {selectedCategoryIds.length
                  ? `${selectedCategoryIds.length} selected`
                  : "All categories"}
              </Badge>
              {showAppliedFilter ? (
                <Badge variant="outline">Filter: {appliedFilter.trim()}</Badge>
              ) : null}
              {open ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={onReset}
                  disabled={!selectedCategoryIds.length || saving}
                >
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
          </div>
        </CardHeader>

        <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          <CardContent className="flex flex-col gap-4 px-0 pb-0 sm:p-5">
            <div className="relative">
              <SearchIcon
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                className="pl-10"
                placeholder="Filter categories"
                value={filterInput}
                onChange={(event) => onFilterInputChange(event.target.value)}
                onKeyDown={handleFilterKeyDown}
                disabled={!preferencesReady}
              />
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Toggle
                variant="outline"
                pressed={showOnlyChannelsWithEpg}
                onPressedChange={onToggleEpgOnly}
                aria-label="Hide channels without EPG"
              >
                Hide channels without EPG
              </Toggle>
            </div>

            <ScrollArea
              type="always"
              className="h-[28rem] rounded-none border-0 bg-transparent sm:rounded-2xl sm:border sm:border-border/70 sm:bg-background/70"
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
                        data-guide-category-filter="true"
                        aria-pressed={selected}
                        disabled={!preferencesReady || saving}
                        onClick={() => onToggleCategory(category.id)}
                        className={cn(
                          "flex items-center justify-between gap-3 rounded-xl px-3 py-3 text-left transition-colors",
                          selected
                            ? "bg-primary/10 text-foreground"
                            : "text-muted-foreground hover:bg-accent hover:text-foreground",
                        )}
                      >
                        <div className="flex min-w-0 items-center gap-3">
                          <div
                            className={cn(
                              "flex size-6 shrink-0 items-center justify-center rounded-full border",
                              selected
                                ? "border-primary bg-primary text-primary-foreground"
                                : "border-border bg-background",
                            )}
                          >
                            {selected ? (
                              <Check className="size-3.5" aria-hidden="true" />
                            ) : null}
                          </div>
                          <span className="truncate font-medium">
                            {category.name}
                          </span>
                        </div>
                        <div className="flex shrink-0 flex-col items-end gap-1 sm:flex-row sm:items-center sm:gap-2">
                          <Badge variant="outline">
                            {category.channelCount}
                          </Badge>
                          <Badge
                            variant={category.liveNowCount ? "live" : "outline"}
                          >
                            {category.liveNowCount}
                          </Badge>
                        </div>
                      </button>
                    );
                  })
                ) : (
                  <div className="px-4 py-8 text-center text-sm text-muted-foreground">
                    {showAppliedFilter
                      ? "No matching categories"
                      : "No categories"}
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

export function GuideCategorySection({
  category,
  open,
  favoritePending,
  activeFavoriteChannelId,
  categoryFavoritePending,
  activeFavoriteCategoryId,
  showOnlyChannelsWithEpg,
  onToggle,
  onToggleCategoryFavorite,
  onFavorite,
  onPlay,
}: GuideCategorySectionProps) {
  const categoryQuery = useInfiniteQuery({
    queryKey: ["guide", "category", category.id, { withEpgOnly: showOnlyChannelsWithEpg }],
    queryFn: ({ pageParam }) =>
      getGuideCategory(
        category.id,
        pageParam,
        GUIDE_PAGE_SIZE,
        showOnlyChannelsWithEpg,
      ),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: open,
  });
  const entries =
    categoryQuery.data?.pages.flatMap((page) => page.entries) ?? [];
  const hasEntries = entries.length > 0;
  const isInitialLoading = open && categoryQuery.isLoading && !hasEntries;
  const Icon = open ? ChevronDown : ChevronRight;

  return (
    <Collapsible open={open} onOpenChange={onToggle}>
      <div className="flex flex-col gap-4 px-0 py-4 sm:p-5">
        <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <div className="flex min-w-0 flex-1 flex-col gap-3">
            <div className="flex min-w-0 flex-col gap-2">
              <h2 className="min-w-0 text-lg font-semibold break-words">
                {category.name}
              </h2>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline">
                  {category.channelCount} channels
                </Badge>
                <Badge variant={category.liveNowCount ? "live" : "outline"}>
                  {category.liveNowCount} live now
                </Badge>
              </div>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="ghost"
              className="self-start"
              onClick={() => onToggleCategoryFavorite(category)}
              disabled={
                categoryFavoritePending
                && activeFavoriteCategoryId === category.id
              }
            >
              <Heart
                data-icon="inline-start"
                className={category.isFavorite ? "fill-current opacity-70" : "opacity-70"}
              />
              {category.isFavorite ? "Unfavorite category" : "Favorite category"}
            </Button>
            <CollapsibleTrigger asChild>
              <Button
                variant="ghost"
                className="self-start"
                aria-expanded={open}
                data-guide-category-toggle="true"
              >
                <Icon data-icon="inline-start" />
                {open ? "Hide channels" : "Show channels"}
              </Button>
            </CollapsibleTrigger>
          </div>
        </div>
      </div>

      <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
        <Separator />
        <div className="flex flex-col">
          {isInitialLoading ? (
            <div className="flex flex-col gap-3 px-0 py-4 sm:p-5">
              {Array.from({ length: 3 }).map((_, index) => (
                <div
                  key={index}
                  className="flex items-center gap-4 rounded-2xl border border-border/70 p-4"
                >
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
            <div className="p-5 text-sm text-destructive">
              Unable to load this category right now.
            </div>
          ) : null}

          {hasEntries
            ? entries.map(({ channel, program }, index) => {
                const programIsLive = program
                  ? isProgramLive(program.startAt, program.endAt)
                  : false;

                return (
                  <div key={channel.id} className="group">
                    {index > 0 ? <Separator /> : null}
                    <div className="flex flex-col gap-4 p-4 transition-colors hover:bg-muted/30 sm:gap-5 sm:p-5">
                      <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
                        <div className="flex min-w-0 flex-1 items-start gap-3 sm:gap-4">
                          <ChannelAvatar
                            name={channel.name}
                            logoUrl={channel.logoUrl}
                            className="h-12 w-12 shrink-0 rounded-xl ring-1 ring-border/10 sm:h-14 sm:w-14 sm:rounded-2xl"
                            fallbackClassName="rounded-xl sm:rounded-2xl"
                          />
                          <div className="flex min-w-0 flex-1 flex-col gap-1.5 pt-0.5">
                            <div className="flex min-w-0 flex-wrap items-center gap-2">
                              <h3 className="min-w-0 break-words text-base font-semibold tracking-tight sm:text-lg">
                                {channel.name}
                              </h3>
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
                                  {formatArchiveDuration(
                                    channel.archiveDurationHours,
                                  )}
                                </Badge>
                              ) : null}
                            </div>
                          </div>
                        </div>

                        <div className="hidden shrink-0 items-center gap-2 pt-1 sm:flex">
                          <Button
                            variant="ghost"
                            size="sm"
                            className="text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
                            onClick={() => onFavorite(channel)}
                            disabled={
                              favoritePending
                              && activeFavoriteChannelId === channel.id
                            }
                          >
                            <Heart data-icon="inline-start" className={channel.isFavorite ? "fill-current opacity-70" : "opacity-70"} />
                            {channel.isFavorite ? "Unfavorite" : "Favorite"}
                          </Button>
                          <Button
                            size="sm"
                            className="min-w-24 shadow-sm"
                            onClick={() => onPlay(channel.id)}
                          >
                            <Play data-icon="inline-start" />
                            Play
                          </Button>
                        </div>
                      </div>

                      <div className="flex w-full items-center gap-2 sm:hidden">
                        <Button
                          variant="secondary"
                          className="flex-1 bg-secondary/50 shadow-sm"
                          onClick={() => onFavorite(channel)}
                          disabled={
                            favoritePending
                            && activeFavoriteChannelId === channel.id
                          }
                        >
                          <Heart data-icon="inline-start" className={channel.isFavorite ? "fill-current opacity-70" : "opacity-70"} />
                          {channel.isFavorite ? "Unfavorite" : "Favorite"}
                        </Button>
                        <Button
                          className="flex-1 shadow-sm"
                          onClick={() => onPlay(channel.id)}
                        >
                          <Play data-icon="inline-start" />
                          Play
                        </Button>
                      </div>

                      <div className="rounded-xl border border-border/40 bg-secondary/20 p-3.5 sm:p-4">
                        {program ? (
                          <div className="flex min-w-0 flex-col gap-2">
                            <div className="flex flex-wrap items-start gap-x-2 gap-y-1">
                              <p className="min-w-0 break-words text-sm font-semibold leading-6">
                                {program.title || "No program"}
                              </p>
                              {programIsLive ? (
                                <Badge variant="accent">Live now</Badge>
                              ) : null}
                            </div>
                            <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                              <span className="font-medium text-foreground/80">
                                {formatTimeRange(
                                  program.startAt,
                                  program.endAt,
                                )}
                              </span>
                              {program.canCatchup ? (
                                <Badge variant="outline" className="h-5 px-1.5 py-0 text-[10px] font-medium uppercase tracking-widest opacity-80">
                                  Catch-up window
                                </Badge>
                              ) : null}
                            </div>
                            {programIsLive ? (
                              <Progress
                                value={getTimeProgress(
                                  program.startAt,
                                  program.endAt,
                                )}
                                className="mt-2 h-1.5 bg-border/50"
                              />
                            ) : null}
                          </div>
                        ) : (
                          <p className="text-sm font-medium text-muted-foreground">No program data</p>
                        )}
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
                  {categoryQuery.isFetchingNextPage
                    ? "Loading more..."
                    : "Load more"}
                </Button>
              </div>
            </>
          ) : null}

          {!isInitialLoading
          && open
          && !hasEntries
          && !categoryQuery.isError ? (
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

function isProgramLive(startAt: string, endAt: string) {
  const now = Date.now();
  return new Date(startAt).getTime() <= now && new Date(endAt).getTime() > now;
}
