import type {
  Channel,
  Program,
  SearchBackend,
  SearchFilterOptionsResponse,
} from "@euripus/shared";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import {
  Clapperboard,
  Filter,
  Heart,
  Play,
  Search as SearchIcon,
  TvMinimal,
} from "lucide-react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useDebounce } from "@/hooks/use-debounce";
import {
  useChannelPlaybackMutation,
  useProgramPlaybackMutation,
} from "@/hooks/use-playback-actions";
import {
  getSearchFilterOptions,
  searchChannels,
  searchPrograms,
} from "@/lib/api";
import { SEARCH_QUERY_STALE_TIME_MS } from "@/lib/query-cache";
import {
  canPlayProgram,
  cn,
  formatEventChannelTitle,
  formatTimeRange,
  getProgramPlaybackState,
  type ProgramPlaybackState,
} from "@/lib/utils";

const SEARCH_PAGE_SIZE = 30;
const HEAVY_ROW_STYLE = {
  contentVisibility: "auto",
  containIntrinsicSize: "120px",
} as const;

export function SearchPage() {
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<"channels" | "programs">(
    "channels",
  );
  const [selectionStart, setSelectionStart] = useState(0);
  const [isSearchInputFocused, setIsSearchInputFocused] = useState(false);
  const [highlightedOptionIndex, setHighlightedOptionIndex] = useState(0);
  const [autocompleteDismissed, setAutocompleteDismissed] = useState(false);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const debouncedQuery = useDebounce(query, 250);
  const hasQuery = debouncedQuery.trim().length > 1;
  const filterOptionsQuery = useQuery({
    queryKey: ["search", "filter-options"],
    queryFn: getSearchFilterOptions,
    staleTime: SEARCH_QUERY_STALE_TIME_MS,
  });
  const channelQuery = useInfiniteQuery({
    queryKey: ["search", "channels", debouncedQuery],
    queryFn: ({ pageParam }) =>
      searchChannels(debouncedQuery, pageParam, SEARCH_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: hasQuery,
    staleTime: SEARCH_QUERY_STALE_TIME_MS,
  });
  const programQuery = useInfiniteQuery({
    queryKey: ["search", "programs", debouncedQuery],
    queryFn: ({ pageParam }) =>
      searchPrograms(debouncedQuery, pageParam, SEARCH_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: hasQuery,
    staleTime: SEARCH_QUERY_STALE_TIME_MS,
  });
  const favoriteMutation = useChannelFavoriteMutation();
  const playChannelMutation = useChannelPlaybackMutation();
  const playProgramMutation = useProgramPlaybackMutation();

  const channels = useMemo(
    () => channelQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [channelQuery.data],
  );
  const programs = useMemo(
    () => programQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [programQuery.data],
  );
  const channelTotal = channelQuery.data?.pages[0]?.totalCount ?? 0;
  const programTotal = programQuery.data?.pages[0]?.totalCount ?? 0;
  const channelBackend = channelQuery.data?.pages[0]?.backend;
  const programBackend = programQuery.data?.pages[0]?.backend;
  const totalMatches = channelTotal + programTotal;
  const isInitialLoading =
    hasQuery &&
    ((channelQuery.isPending && !channels.length) ||
      (programQuery.isPending && !programs.length));
  const headerMeta = useMemo(
    () =>
      hasQuery ? (
        <>
          <Badge variant="accent">{totalMatches} matches</Badge>
          <Badge variant="outline">{channelTotal} channels</Badge>
          <Badge variant="outline">{programTotal} programs</Badge>
        </>
      ) : null,
    [channelTotal, hasQuery, programTotal, totalMatches],
  );
  const guideState = useMemo(() => buildSearchGuideState(query), [query]);
  const autocompleteState = useMemo(
    () => getSearchAutocompleteState(query, selectionStart),
    [query, selectionStart],
  );
  const autocompleteOptions = useMemo(
    () =>
      getSearchAutocompleteSuggestions(
        filterOptionsQuery.data,
        autocompleteState,
        guideState.countries,
      ),
    [autocompleteState, filterOptionsQuery.data, guideState.countries],
  );
  const isAutocompleteOpen =
    isSearchInputFocused &&
    autocompleteState !== null &&
    !autocompleteDismissed &&
    !filterOptionsQuery.isError;

  useEffect(() => {
    setHighlightedOptionIndex(0);
    setAutocompleteDismissed(false);
  }, [
    autocompleteState?.end,
    autocompleteState?.kind,
    autocompleteState?.start,
    autocompleteState?.value,
  ]);

  useEffect(() => {
    if (highlightedOptionIndex >= autocompleteOptions.length) {
      setHighlightedOptionIndex(0);
    }
  }, [autocompleteOptions.length, highlightedOptionIndex]);

  useEffect(() => {
    if (!hasQuery) {
      setActiveTab("channels");
      return;
    }

    if (!channelTotal && programTotal) {
      setActiveTab("programs");
      return;
    }

    if (channelTotal) {
      setActiveTab("channels");
    }
  }, [channelTotal, hasQuery, programTotal]);

  const updateSearchCursor = useCallback((input: HTMLInputElement) => {
    const nextSelectionStart = input.selectionStart ?? input.value.length;
    setSelectionStart(nextSelectionStart);
  }, []);

  const applyAutocompleteOption = useCallback(
    (option: string) => {
      if (!autocompleteState) {
        return;
      }

      const nextState = applySearchAutocompleteOption(
        query,
        autocompleteState,
        option,
      );
      setQuery(nextState.query);
      setSelectionStart(nextState.cursor);

      requestAnimationFrame(() => {
        searchInputRef.current?.focus();
        searchInputRef.current?.setSelectionRange(
          nextState.cursor,
          nextState.cursor,
        );
      });
    },
    [autocompleteState, query],
  );

  const handleSearchKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (!isAutocompleteOpen || filterOptionsQuery.isPending) {
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        setAutocompleteDismissed(true);
        return;
      }

      if (!autocompleteOptions.length) {
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setHighlightedOptionIndex((current) =>
          current >= autocompleteOptions.length - 1 ? 0 : current + 1,
        );
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        setHighlightedOptionIndex((current) =>
          current <= 0 ? autocompleteOptions.length - 1 : current - 1,
        );
        return;
      }

      if (event.key === "Enter") {
        event.preventDefault();
        applyAutocompleteOption(
          autocompleteOptions[highlightedOptionIndex] ?? autocompleteOptions[0],
        );
      }
    },
    [
      applyAutocompleteOption,
      autocompleteOptions,
      filterOptionsQuery.isPending,
      highlightedOptionIndex,
      isAutocompleteOpen,
    ],
  );

  const handleToggleFavorite = useCallback(
    (channel: Channel) => favoriteMutation.mutate(channel),
    [favoriteMutation],
  );
  const handlePlayChannel = useCallback(
    (channelId: string) => playChannelMutation.mutate(channelId),
    [playChannelMutation],
  );
  const handlePlayProgram = useCallback(
    (program: Program, playbackState: ProgramPlaybackState) => {
      if (playbackState === "live" && program.channelId) {
        playChannelMutation.mutate(program.channelId);
        return;
      }

      playProgramMutation.mutate(program.id);
    },
    [playChannelMutation, playProgramMutation],
  );

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Master Search" meta={headerMeta} />

      <Card className="rounded-none border-0 bg-transparent shadow-none sm:border-border/80 sm:bg-gradient-to-r sm:from-card sm:via-card sm:to-primary/5 sm:shadow-sm">
        <CardContent className="px-0 pt-0 pb-0 sm:px-6 sm:pt-5 sm:pb-6">
          <div className="grid gap-3">
            <Popover open={isAutocompleteOpen}>
              <PopoverAnchor asChild>
                <div className="relative">
                  <SearchIcon
                    className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
                    aria-hidden="true"
                  />
                  <Input
                    ref={searchInputRef}
                    data-search-input="true"
                    className="pl-10"
                    placeholder="Search"
                    value={query}
                    onChange={(event) => {
                      setQuery(event.target.value);
                      updateSearchCursor(event.target);
                    }}
                    onFocus={(event) => {
                      setIsSearchInputFocused(true);
                      updateSearchCursor(event.target);
                    }}
                    onBlur={() => setIsSearchInputFocused(false)}
                    onClick={(event) => updateSearchCursor(event.currentTarget)}
                    onKeyUp={(event) => updateSearchCursor(event.currentTarget)}
                    onSelect={(event) => updateSearchCursor(event.currentTarget)}
                    onKeyDown={handleSearchKeyDown}
                  />
                </div>
              </PopoverAnchor>
              <PopoverContent
                align="start"
                sideOffset={8}
                className="w-[min(24rem,calc(100vw-2rem))] p-0"
                onOpenAutoFocus={(event) => event.preventDefault()}
              >
                <Command shouldFilter={false}>
                  {filterOptionsQuery.isPending ? (
                    <div className="px-3 py-4 text-sm text-muted-foreground">
                      Loading available filters...
                    </div>
                  ) : (
                    <CommandList>
                      <CommandEmpty>
                        No {autocompleteState?.kind === "country" ? "countries" : "providers"} match that token yet.
                      </CommandEmpty>
                      <CommandGroup
                        heading={
                          autocompleteState?.kind === "country"
                            ? "Countries"
                            : "Providers"
                        }
                      >
                        {autocompleteOptions.map((option, index) => (
                          <CommandItem
                            key={option}
                            value={option}
                            className={cn(
                              "flex items-center justify-between gap-3",
                              index === highlightedOptionIndex &&
                                "bg-accent text-accent-foreground",
                            )}
                            onMouseDown={(event) => event.preventDefault()}
                            onSelect={() => applyAutocompleteOption(option)}
                          >
                            <span>{option}</span>
                            <span className="text-xs text-muted-foreground">
                              {autocompleteState?.kind}:{option}
                            </span>
                          </CommandItem>
                        ))}
                      </CommandGroup>
                    </CommandList>
                  )}
                </Command>
              </PopoverContent>
            </Popover>

            <SearchGuide
              state={guideState}
              onApplySuggestion={(suggestion) =>
                setQuery((current) => applySearchSuggestion(current, suggestion))
              }
            />
          </div>
        </CardContent>
      </Card>

      {!hasQuery ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <SearchIcon aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>Start typing to search</EmptyTitle>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {isInitialLoading ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="flex flex-col gap-3 px-0 pt-0 pb-0 sm:px-6 sm:pt-5 sm:pb-6">
            {Array.from({ length: 5 }).map((_, index) => (
              <div
                key={index}
                className="flex items-center gap-4 rounded-xl border border-border/70 p-4"
              >
                <Skeleton className="size-11 rounded-2xl" />
                <div className="flex flex-1 flex-col gap-2">
                  <Skeleton className="h-4 w-44" />
                  <Skeleton className="h-4 w-64" />
                </div>
                <Skeleton className="h-9 w-24 rounded-lg" />
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}

      <SearchResults
        activeTab={activeTab}
        channelBackend={channelBackend}
        channelTotal={channelTotal}
        channels={channels}
        channelsHasNextPage={channelQuery.hasNextPage}
        channelsLoadingMore={channelQuery.isFetchingNextPage}
        favoritePending={favoriteMutation.isPending}
        favoritePendingChannelId={favoriteMutation.variables?.id}
        hasQuery={hasQuery}
        isInitialLoading={isInitialLoading}
        onActiveTabChange={setActiveTab}
        onLoadMoreChannels={() => channelQuery.fetchNextPage()}
        onLoadMorePrograms={() => programQuery.fetchNextPage()}
        onPlayChannel={handlePlayChannel}
        onPlayProgram={handlePlayProgram}
        onToggleFavorite={handleToggleFavorite}
        playPending={playChannelMutation.isPending || playProgramMutation.isPending}
        programBackend={programBackend}
        programTotal={programTotal}
        programs={programs}
        programsHasNextPage={programQuery.hasNextPage}
        programsLoadingMore={programQuery.isFetchingNextPage}
      />
    </div>
  );
}

const ChannelSearchRow = memo(function ChannelSearchRow({
  channel,
  favoritePending,
  playPending,
  onPlay,
  onToggleFavorite,
}: {
  channel: Channel;
  favoritePending: boolean;
  playPending: boolean;
  onPlay: (channelId: string) => void;
  onToggleFavorite: (channel: Channel) => void;
}) {
  const displayChannelName = formatEventChannelTitle(channel.name);

  return (
    <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
      <div className="flex min-w-0 items-start gap-4">
        <ChannelAvatar name={displayChannelName} logoUrl={channel.logoUrl} />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="truncate text-base font-semibold">{displayChannelName}</h2>
            {channel.categoryName ? (
              <Badge variant="outline">{channel.categoryName}</Badge>
            ) : null}
            {channel.hasEpg ? <Badge variant="outline">EPG</Badge> : null}
            {channel.hasCatchup ? <Badge variant="live">Catch-up</Badge> : null}
            {channel.isFavorite ? <Badge variant="accent">Favorite</Badge> : null}
          </div>
          {channel.streamExtension ? (
            <p className="text-sm text-muted-foreground">
              {channel.streamExtension.toUpperCase()}
            </p>
          ) : null}
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => onToggleFavorite(channel)}
          disabled={favoritePending}
        >
          <Heart data-icon="inline-start" />
          {channel.isFavorite ? "Unfavorite" : "Favorite"}
        </Button>
        <Button size="sm" onClick={() => onPlay(channel.id)} disabled={playPending}>
          <Play data-icon="inline-start" />
          Play
        </Button>
      </div>
    </div>
  );
});

const ProgramSearchRow = memo(function ProgramSearchRow({
  program,
  playPending,
  onPlay,
}: {
  program: Program;
  playPending: boolean;
  onPlay: (program: Program, playbackState: ProgramPlaybackState) => void;
}) {
  const playbackState = getProgramPlaybackState(program);
  const displayChannelName = program.channelName
    ? formatEventChannelTitle(program.channelName, {
        referenceStartAt: program.startAt,
      })
    : null;

  return (
    <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
      <div className="flex min-w-0 items-start gap-4">
        <ChannelAvatar
          name={displayChannelName ?? program.title}
          className="size-10"
        />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="truncate text-base font-semibold">{program.title}</h2>
            {displayChannelName ? (
              <Badge variant="outline">{displayChannelName}</Badge>
            ) : null}
            <ProgramStateBadge state={playbackState} />
          </div>
          {program.description ? (
            <p className="text-sm text-muted-foreground">{program.description}</p>
          ) : null}
        </div>
      </div>
      <div className="flex items-center gap-3 self-start lg:self-center">
        <Badge variant="outline">
          {formatTimeRange(program.startAt, program.endAt)}
        </Badge>
        {canPlayProgram(program) ? (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onPlay(program, playbackState)}
            disabled={playPending}
          >
            <Play data-icon="inline-start" />
            Play
          </Button>
        ) : (
          <span className="text-sm text-muted-foreground">
            {playbackState === "upcoming" ? "Upcoming only" : "Info only"}
          </span>
        )}
      </div>
    </div>
  );
});

const SearchResults = memo(function SearchResults({
  activeTab,
  channelBackend,
  channelTotal,
  channels,
  channelsHasNextPage,
  channelsLoadingMore,
  favoritePending,
  favoritePendingChannelId,
  hasQuery,
  isInitialLoading,
  onActiveTabChange,
  onLoadMoreChannels,
  onLoadMorePrograms,
  onPlayChannel,
  onPlayProgram,
  onToggleFavorite,
  playPending,
  programBackend,
  programTotal,
  programs,
  programsHasNextPage,
  programsLoadingMore,
}: {
  activeTab: "channels" | "programs";
  channelBackend?: SearchBackend;
  channelTotal: number;
  channels: Channel[];
  channelsHasNextPage: boolean;
  channelsLoadingMore: boolean;
  favoritePending: boolean;
  favoritePendingChannelId?: string;
  hasQuery: boolean;
  isInitialLoading: boolean;
  onActiveTabChange: (value: "channels" | "programs") => void;
  onLoadMoreChannels: () => void;
  onLoadMorePrograms: () => void;
  onPlayChannel: (channelId: string) => void;
  onPlayProgram: (program: Program, playbackState: ProgramPlaybackState) => void;
  onToggleFavorite: (channel: Channel) => void;
  playPending: boolean;
  programBackend?: SearchBackend;
  programTotal: number;
  programs: Program[];
  programsHasNextPage: boolean;
  programsLoadingMore: boolean;
}) {
  if (!hasQuery || isInitialLoading) {
    return null;
  }

  const useDeferredRowPaint = channels.length > 40 || programs.length > 40;

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => onActiveTabChange(value as "channels" | "programs")}
      className="flex flex-col gap-4"
    >
      <TabsList>
        <TabsTrigger value="channels">
          <span>Channels ({channelTotal})</span>
          <SearchBackendBadge backend={channelBackend} compact />
        </TabsTrigger>
        <TabsTrigger value="programs">
          <span>Programs ({programTotal})</span>
          <SearchBackendBadge backend={programBackend} compact />
        </TabsTrigger>
      </TabsList>

      {activeTab === "channels" ? (
        <TabsContent value="channels" className="mt-0">
          <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
            <CardHeader className="px-0 pt-0 pb-4 sm:px-6 sm:pt-6 sm:pb-0">
              <div className="flex items-center justify-between gap-3">
                <CardTitle>Channel matches</CardTitle>
                <SearchBackendBadge backend={channelBackend} />
              </div>
            </CardHeader>
            <CardContent className="p-0">
              {channels.length ? (
                <>
                  {channels.map((channel, index) => (
                    <div
                      key={channel.id}
                      style={useDeferredRowPaint ? HEAVY_ROW_STYLE : undefined}
                    >
                      {index > 0 ? <Separator /> : null}
                      <ChannelSearchRow
                        channel={channel}
                        favoritePending={
                          favoritePending && favoritePendingChannelId === channel.id
                        }
                        playPending={playPending}
                        onPlay={onPlayChannel}
                        onToggleFavorite={onToggleFavorite}
                      />
                    </div>
                  ))}
                  <LoadMoreTrigger
                    hasNextPage={channelsHasNextPage}
                    isFetchingNextPage={channelsLoadingMore}
                    onLoadMore={onLoadMoreChannels}
                  />
                </>
              ) : (
                <Empty className="border-0">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">
                      <TvMinimal aria-hidden="true" />
                    </EmptyMedia>
                    <EmptyTitle>No channel matches</EmptyTitle>
                  </EmptyHeader>
                </Empty>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      ) : null}

      {activeTab === "programs" ? (
        <TabsContent value="programs" className="mt-0">
          <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
            <CardHeader className="px-0 pt-0 pb-4 sm:px-6 sm:pt-6 sm:pb-0">
              <div className="flex items-center justify-between gap-3">
                <CardTitle>EPG matches</CardTitle>
                <SearchBackendBadge backend={programBackend} />
              </div>
            </CardHeader>
            <CardContent className="p-0">
              {programs.length ? (
                <>
                  {programs.map((program, index) => (
                    <div
                      key={program.id}
                      style={useDeferredRowPaint ? HEAVY_ROW_STYLE : undefined}
                    >
                      {index > 0 ? <Separator /> : null}
                      <ProgramSearchRow
                        program={program}
                        playPending={playPending}
                        onPlay={onPlayProgram}
                      />
                    </div>
                  ))}
                  <LoadMoreTrigger
                    hasNextPage={programsHasNextPage}
                    isFetchingNextPage={programsLoadingMore}
                    onLoadMore={onLoadMorePrograms}
                  />
                </>
              ) : (
                <Empty className="border-0">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">
                      <Clapperboard aria-hidden="true" />
                    </EmptyMedia>
                    <EmptyTitle>No EPG matches</EmptyTitle>
                  </EmptyHeader>
                </Empty>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      ) : null}
    </Tabs>
  );
});

type SearchGuideState = {
  freeText: string;
  countries: string[];
  providers: string[];
  ppv: boolean | null;
  vip: boolean | null;
  requireEpg: boolean;
  suggestions: string[];
};

type SearchAutocompleteKind = "country" | "provider";

type SearchAutocompleteState = {
  kind: SearchAutocompleteKind;
  start: number;
  end: number;
  value: string;
};

function getSearchAutocompleteState(
  query: string,
  selectionStart: number,
): SearchAutocompleteState | null {
  const cursor = Math.min(selectionStart, query.length);
  let start = cursor;
  while (start > 0 && !/\s/.test(query[start - 1] ?? "")) {
    start -= 1;
  }

  let end = cursor;
  while (end < query.length && !/\s/.test(query[end] ?? "")) {
    end += 1;
  }

  const token = query.slice(start, end);
  const normalizedToken = token.toLowerCase();
  if (normalizedToken.startsWith("country:")) {
    return {
      kind: "country",
      start,
      end,
      value: token.slice("country:".length).trim().toLowerCase(),
    };
  }

  if (normalizedToken.startsWith("provider:")) {
    return {
      kind: "provider",
      start,
      end,
      value: token.slice("provider:".length).trim().toLowerCase(),
    };
  }

  return null;
}

function getSearchAutocompleteSuggestions(
  options: SearchFilterOptionsResponse | undefined,
  state: SearchAutocompleteState | null,
  selectedCountries: string[],
) {
  if (!state) {
    return [];
  }

  const source =
    state.kind === "country"
      ? options?.countries ?? []
      : (options?.providers ?? [])
          .filter(
            (provider) =>
              selectedCountries.length === 0 ||
              provider.countryCodes.some((countryCode) =>
                selectedCountries.includes(countryCode),
              ),
          )
          .map((provider) => provider.value);

  return source
    .filter((option) => !state.value || option.includes(state.value))
    .slice(0, 20);
}

function applySearchAutocompleteOption(
  currentQuery: string,
  state: SearchAutocompleteState,
  option: string,
) {
  const replacement = `${state.kind}:${option.toLowerCase()}`;
  const before = currentQuery.slice(0, state.start);
  const after = currentQuery.slice(state.end);
  const needsTrailingSpace = after.length === 0;
  const nextQuery = `${before}${replacement}${needsTrailingSpace ? " " : ""}${after}`;
  const cursor = before.length + replacement.length + (needsTrailingSpace ? 1 : 0);

  return {
    query: nextQuery,
    cursor,
  };
}

function SearchGuide({
  state,
  onApplySuggestion,
}: {
  state: SearchGuideState;
  onApplySuggestion: (suggestion: string) => void;
}) {
  const hasActiveFilters =
    state.countries.length > 0 ||
    state.providers.length > 0 ||
    state.ppv !== null ||
    state.vip !== null ||
    state.requireEpg;

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-border/70 bg-background/70 px-3 py-3">
      <div className="flex items-start gap-2">
        <Filter className="mt-0.5 size-4 text-muted-foreground" aria-hidden="true" />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <p className="text-sm text-muted-foreground">
            {hasActiveFilters ? (
              <span>
                Search guide: {describeSearchGuideState(state)}
              </span>
            ) : (
              <span>
                Search guide: combine free text with filters like <code>country:se</code>,{" "}
                <code>provider:viaplay</code>, <code>ppv</code>, <code>!ppv</code>,{" "}
                <code>vip</code>, <code>!vip</code>, or <code>epg</code>.
              </span>
            )}
          </p>

          <div className="flex flex-wrap gap-2">
            {state.suggestions.map((suggestion) => (
              <Button
                key={suggestion}
                type="button"
                variant="outline"
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={() => onApplySuggestion(suggestion)}
              >
                {suggestion}
              </Button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function buildSearchGuideState(query: string): SearchGuideState {
  const parsed = parseSearchGuideQuery(query);
  const suggestions = new Set<string>();

  if (!parsed.freeText) {
    suggestions.add("country:se");
    suggestions.add("provider:viaplay");
  }

  if (!parsed.requireEpg) {
    suggestions.add("epg");
  }

  if (parsed.ppv === null) {
    suggestions.add("ppv");
    suggestions.add("!ppv");
  }

  if (parsed.vip === null) {
    suggestions.add("vip");
    suggestions.add("!vip");
  }

  if (parsed.countries.length === 0) {
    suggestions.add("country:se");
  }

  if (parsed.providers.length === 0) {
    suggestions.add("provider:viaplay");
  }

  return {
    ...parsed,
    suggestions: Array.from(suggestions).slice(0, 6),
  };
}

function parseSearchGuideQuery(query: string) {
  const countries: string[] = [];
  const providers: string[] = [];
  let ppv: boolean | null = null;
  let vip: boolean | null = null;
  let requireEpg = false;

  const freeText = query
    .split(/\s+/)
    .filter(Boolean)
    .flatMap((token) => {
      const normalizedToken = token.trim().toLowerCase();
      if (!normalizedToken) {
        return [];
      }

      if (normalizedToken.startsWith("country:")) {
        const value = normalizedToken.slice("country:".length).trim();
        if (value && !countries.includes(value)) {
          countries.push(value);
        }
        return [];
      }

      if (normalizedToken.startsWith("provider:")) {
        const value = normalizedToken.slice("provider:".length).trim();
        if (value && !providers.includes(value)) {
          providers.push(value);
        }
        return [];
      }

      if (normalizedToken === "ppv") {
        ppv = true;
        return [];
      }

      if (normalizedToken === "!ppv") {
        ppv = false;
        return [];
      }

      if (normalizedToken === "vip") {
        vip = true;
        return [];
      }

      if (normalizedToken === "!vip") {
        vip = false;
        return [];
      }

      if (normalizedToken === "epg") {
        requireEpg = true;
        return [];
      }

      return [token];
    })
    .join(" ")
    .trim();

  return {
    freeText,
    countries,
    providers,
    ppv,
    vip,
    requireEpg,
  };
}

function describeSearchGuideState(state: SearchGuideState) {
  const parts: string[] = [];

  if (state.freeText) {
    parts.push(`searching for "${state.freeText}"`);
  }
  if (state.countries.length) {
    parts.push(`country ${state.countries.join(", ")}`);
  }
  if (state.providers.length) {
    parts.push(`provider ${state.providers.join(", ")}`);
  }
  if (state.ppv === true) {
    parts.push("PPV only");
  }
  if (state.ppv === false) {
    parts.push("excluding PPV");
  }
  if (state.vip === true) {
    parts.push("VIP only");
  }
  if (state.vip === false) {
    parts.push("excluding VIP");
  }
  if (state.requireEpg) {
    parts.push("EPG only");
  }

  return parts.join(" · ");
}

function applySearchSuggestion(currentQuery: string, suggestion: string) {
  const tokens = currentQuery
    .split(/\s+/)
    .map((token) => token.trim())
    .filter(Boolean);

  const nextTokens = tokens.filter((token) => {
    const normalizedToken = token.toLowerCase();
    const normalizedSuggestion = suggestion.toLowerCase();

    if (normalizedSuggestion.startsWith("country:")) {
      return !normalizedToken.startsWith("country:");
    }

    if (normalizedSuggestion.startsWith("provider:")) {
      return !normalizedToken.startsWith("provider:");
    }

    if (normalizedSuggestion === "ppv" || normalizedSuggestion === "!ppv") {
      return normalizedToken !== "ppv" && normalizedToken !== "!ppv";
    }

    if (normalizedSuggestion === "vip" || normalizedSuggestion === "!vip") {
      return normalizedToken !== "vip" && normalizedToken !== "!vip";
    }

    if (normalizedSuggestion === "epg") {
      return normalizedToken !== "epg";
    }

    return true;
  });

  nextTokens.push(suggestion);
  return nextTokens.join(" ").trim();
}

function ProgramStateBadge({ state }: { state: ProgramPlaybackState }) {
  if (state === "live") {
    return <Badge variant="accent">Live now</Badge>;
  }

  if (state === "catchup") {
    return <Badge variant="live">Catch-up</Badge>;
  }

  if (state === "upcoming") {
    return <Badge variant="outline">Upcoming</Badge>;
  }

  return <Badge variant="outline">Info only</Badge>;
}

function SearchBackendBadge({
  backend,
  compact = false,
}: {
  backend?: SearchBackend;
  compact?: boolean;
}) {
  if (!backend) {
    return null;
  }

  if (backend === "meilisearch") {
    return <Badge variant="accent">{compact ? "Meili" : "Meilisearch"}</Badge>;
  }

  return <Badge variant="outline">{compact ? "Postgres" : "PostgreSQL fallback"}</Badge>;
}

function LoadMoreTrigger({
  hasNextPage,
  isFetchingNextPage,
  onLoadMore,
}: {
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  onLoadMore: () => void;
}) {
  const triggerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!hasNextPage || isFetchingNextPage || !triggerRef.current) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          onLoadMore();
        }
      },
      { rootMargin: "160px" },
    );

    observer.observe(triggerRef.current);
    return () => {
      observer.disconnect();
    };
  }, [hasNextPage, isFetchingNextPage, onLoadMore]);

  if (!hasNextPage) {
    return null;
  }

  return (
    <>
      <Separator />
      <div ref={triggerRef} className="p-5">
        <Button
          variant="outline"
          onClick={onLoadMore}
          disabled={isFetchingNextPage}
        >
          {isFetchingNextPage ? "Loading more..." : "Load more"}
        </Button>
      </div>
    </>
  );
}
