import { useInfiniteQuery } from "@tanstack/react-query";
import {
  Clapperboard,
  Heart,
  Play,
  Search as SearchIcon,
  TvMinimal,
} from "lucide-react";
import { useDeferredValue, useEffect, useRef, useState } from "react";
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
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useDebounce } from "@/hooks/use-debounce";
import {
  useChannelPlaybackMutation,
  useProgramPlaybackMutation,
} from "@/hooks/use-playback-actions";
import { searchChannels, searchPrograms } from "@/lib/api";
import {
  canPlayProgram,
  formatTimeRange,
  getProgramPlaybackState,
  type ProgramPlaybackState,
} from "@/lib/utils";

const SEARCH_PAGE_SIZE = 30;

export function SearchPage() {
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<"channels" | "programs">(
    "channels",
  );
  const deferredQuery = useDeferredValue(query);
  const debouncedQuery = useDebounce(deferredQuery, 250);
  const hasQuery = debouncedQuery.trim().length > 1;
  const channelQuery = useInfiniteQuery({
    queryKey: ["search", "channels", debouncedQuery],
    queryFn: ({ pageParam }) =>
      searchChannels(debouncedQuery, pageParam, SEARCH_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: hasQuery,
  });
  const programQuery = useInfiniteQuery({
    queryKey: ["search", "programs", debouncedQuery],
    queryFn: ({ pageParam }) =>
      searchPrograms(debouncedQuery, pageParam, SEARCH_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: hasQuery,
  });
  const favoriteMutation = useChannelFavoriteMutation();
  const playChannelMutation = useChannelPlaybackMutation();
  const playProgramMutation = useProgramPlaybackMutation();

  const channels = channelQuery.data?.pages.flatMap((page) => page.items) ?? [];
  const programs = programQuery.data?.pages.flatMap((page) => page.items) ?? [];
  const channelTotal = channelQuery.data?.pages[0]?.totalCount ?? 0;
  const programTotal = programQuery.data?.pages[0]?.totalCount ?? 0;
  const totalMatches = channelTotal + programTotal;
  const isInitialLoading =
    hasQuery &&
    ((channelQuery.isPending && !channels.length) ||
      (programQuery.isPending && !programs.length));

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

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Master Search"
        meta={
          hasQuery ? (
            <>
              <Badge variant="accent">{totalMatches} matches</Badge>
              <Badge variant="outline">{channelTotal} channels</Badge>
              <Badge variant="outline">{programTotal} programs</Badge>
            </>
          ) : null
        }
      />

      <Card className="rounded-none border-0 bg-transparent shadow-none sm:border-border/80 sm:bg-gradient-to-r sm:from-card sm:via-card sm:to-primary/5 sm:shadow-sm">
        <CardContent className="px-0 pt-0 pb-0 sm:px-6 sm:pt-5 sm:pb-6">
          <div className="relative">
            <SearchIcon
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              data-search-input="true"
              className="pl-10"
              placeholder="Search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
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

      {hasQuery && !isInitialLoading ? (
        <Tabs
          value={activeTab}
          onValueChange={(value) =>
            setActiveTab(value as "channels" | "programs")
          }
          className="flex flex-col gap-4"
        >
          <TabsList>
            <TabsTrigger value="channels">
              Channels ({channelTotal})
            </TabsTrigger>
            <TabsTrigger value="programs">
              Programs ({programTotal})
            </TabsTrigger>
          </TabsList>

          <TabsContent value="channels" className="mt-0">
            <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
              <CardHeader className="px-0 pt-0 pb-4 sm:px-6 sm:pt-6 sm:pb-0">
                <CardTitle>Channel matches</CardTitle>
              </CardHeader>
              <CardContent className="p-0">
                {channels.length ? (
                  <>
                    {channels.map((channel, index) => (
                      <div key={channel.id}>
                        {index > 0 ? <Separator /> : null}
                        <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                          <div className="flex min-w-0 items-start gap-4">
                            <ChannelAvatar
                              name={channel.name}
                              logoUrl={channel.logoUrl}
                            />
                            <div className="flex min-w-0 flex-1 flex-col gap-2">
                              <div className="flex flex-wrap items-center gap-2">
                                <h2 className="truncate text-base font-semibold">
                                  {channel.name}
                                </h2>
                                {channel.categoryName ? (
                                  <Badge variant="outline">
                                    {channel.categoryName}
                                  </Badge>
                                ) : null}
                                {channel.hasCatchup ? (
                                  <Badge variant="live">Catch-up</Badge>
                                ) : null}
                                {channel.isFavorite ? (
                                  <Badge variant="accent">Favorite</Badge>
                                ) : null}
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
                              onClick={() => favoriteMutation.mutate(channel)}
                              disabled={
                                favoriteMutation.isPending &&
                                favoriteMutation.variables?.id === channel.id
                              }
                            >
                              <Heart data-icon="inline-start" />
                              {channel.isFavorite ? "Unfavorite" : "Favorite"}
                            </Button>
                            <Button
                              size="sm"
                              onClick={() =>
                                playChannelMutation.mutate(channel.id)
                              }
                              disabled={playChannelMutation.isPending}
                            >
                              <Play data-icon="inline-start" />
                              Play
                            </Button>
                          </div>
                        </div>
                      </div>
                    ))}
                    <LoadMoreTrigger
                      hasNextPage={channelQuery.hasNextPage}
                      isFetchingNextPage={channelQuery.isFetchingNextPage}
                      onLoadMore={() => channelQuery.fetchNextPage()}
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

          <TabsContent value="programs" className="mt-0">
            <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
              <CardHeader className="px-0 pt-0 pb-4 sm:px-6 sm:pt-6 sm:pb-0">
                <CardTitle>EPG matches</CardTitle>
              </CardHeader>
              <CardContent className="p-0">
                {programs.length ? (
                  <>
                    {programs.map((program, index) => {
                      const playbackState = getProgramPlaybackState(program);

                      return (
                        <div key={program.id}>
                          {index > 0 ? <Separator /> : null}
                          <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                            <div className="flex min-w-0 items-start gap-4">
                              <ChannelAvatar
                                name={program.channelName ?? program.title}
                                className="size-10"
                              />
                              <div className="flex min-w-0 flex-1 flex-col gap-2">
                                <div className="flex flex-wrap items-center gap-2">
                                  <h2 className="truncate text-base font-semibold">
                                    {program.title}
                                  </h2>
                                  {program.channelName ? (
                                    <Badge variant="outline">
                                      {program.channelName}
                                    </Badge>
                                  ) : null}
                                  <ProgramStateBadge state={playbackState} />
                                </div>
                                {program.description ? (
                                  <p className="text-sm text-muted-foreground">
                                    {program.description}
                                  </p>
                                ) : null}
                              </div>
                            </div>
                            <div className="flex items-center gap-3 self-start lg:self-center">
                              <Badge variant="outline">
                                {formatTimeRange(
                                  program.startAt,
                                  program.endAt,
                                )}
                              </Badge>
                              {canPlayProgram(program) ? (
                                <Button
                                  size="sm"
                                  variant="secondary"
                                  onClick={() => {
                                    if (
                                      playbackState === "live" &&
                                      program.channelId
                                    ) {
                                      playChannelMutation.mutate(
                                        program.channelId,
                                      );
                                      return;
                                    }
                                    playProgramMutation.mutate(program.id);
                                  }}
                                  disabled={
                                    playChannelMutation.isPending ||
                                    playProgramMutation.isPending
                                  }
                                >
                                  <Play data-icon="inline-start" />
                                  Play
                                </Button>
                              ) : (
                                <span className="text-sm text-muted-foreground">
                                  {playbackState === "upcoming"
                                    ? "Upcoming only"
                                    : "Info only"}
                                </span>
                              )}
                            </div>
                          </div>
                        </div>
                      );
                    })}
                    <LoadMoreTrigger
                      hasNextPage={programQuery.hasNextPage}
                      isFetchingNextPage={programQuery.isFetchingNextPage}
                      onLoadMore={() => programQuery.fetchNextPage()}
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
        </Tabs>
      ) : null}
    </div>
  );
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
