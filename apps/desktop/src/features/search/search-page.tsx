import { useMutation, useQuery } from "@tanstack/react-query";
import { Clapperboard, Play, Radio, Search as SearchIcon, TvMinimal } from "lucide-react";
import { useState } from "react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useDebounce } from "@/hooks/use-debounce";
import { searchCatalog, startChannelPlayback, startProgramPlayback } from "@/lib/api";
import {
  canPlayProgram,
  formatTimeRange,
  getProgramPlaybackState,
  type ProgramPlaybackState,
} from "@/lib/utils";
import { usePlayerStore } from "@/store/player-store";

export function SearchPage() {
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebounce(query, 300);
  const hasQuery = debouncedQuery.trim().length > 1;
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const searchQuery = useQuery({
    queryKey: ["search", debouncedQuery],
    queryFn: () => searchCatalog(debouncedQuery),
    enabled: hasQuery,
  });
  const playChannelMutation = useMutation({
    mutationFn: startChannelPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });
  const playProgramMutation = useMutation({
    mutationFn: startProgramPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });

  const channels = searchQuery.data?.channels ?? [];
  const programs = searchQuery.data?.programs ?? [];
  const totalMatches = channels.length + programs.length;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Master Search"
        description="Search channel names and full EPG metadata from one place, then jump into live playback or archived results when available."
        meta={
          hasQuery ? (
            <>
              <Badge variant="accent">{totalMatches} matches</Badge>
              <Badge variant="outline">{channels.length} channels</Badge>
              <Badge variant="outline">{programs.length} programs</Badge>
            </>
          ) : null
        }
      />

      <Card>
        <CardHeader>
          <CardTitle>Search channels and EPG</CardTitle>
          <CardDescription>Try a channel name, team, event, league, category, or program title.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="relative">
            <SearchIcon className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
            <Input
              className="pl-10"
              placeholder="Search channels, titles, events, teams..."
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
        </CardContent>
      </Card>

      {!hasQuery ? (
        <Card>
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <SearchIcon aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>Start typing to search</EmptyTitle>
                <EmptyDescription>
                  Search pulls from channels and synced EPG entries once your query is at least two characters.
                </EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {hasQuery && searchQuery.isPending ? (
        <Card>
          <CardContent className="flex flex-col gap-3 pt-5">
            {Array.from({ length: 5 }).map((_, index) => (
              <div key={index} className="flex items-center gap-4 rounded-xl border border-border/70 p-4">
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

      {hasQuery && !searchQuery.isPending ? (
        <Tabs defaultValue={channels.length ? "channels" : "programs"} className="flex flex-col gap-4">
          <TabsList>
            <TabsTrigger value="channels">Channels ({channels.length})</TabsTrigger>
            <TabsTrigger value="programs">Programs ({programs.length})</TabsTrigger>
          </TabsList>

          <TabsContent value="channels" className="mt-0">
            <Card>
              <CardHeader>
                <CardTitle>Channel matches</CardTitle>
                <CardDescription>Direct matches for channel names and categories.</CardDescription>
              </CardHeader>
              <CardContent className="p-0">
                {channels.length ? (
                  channels.map((channel, index) => (
                    <div key={channel.id}>
                      {index > 0 ? <Separator /> : null}
                      <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                        <div className="flex min-w-0 items-start gap-4">
                          <ChannelAvatar name={channel.name} logoUrl={channel.logoUrl} />
                          <div className="flex min-w-0 flex-1 flex-col gap-2">
                            <div className="flex flex-wrap items-center gap-2">
                              <h2 className="truncate text-base font-semibold">{channel.name}</h2>
                              {channel.categoryName ? <Badge variant="outline">{channel.categoryName}</Badge> : null}
                              {channel.hasCatchup ? <Badge variant="live">Catch-up</Badge> : null}
                            </div>
                            <p className="text-sm text-muted-foreground">
                              {channel.streamExtension
                                ? `${channel.streamExtension.toUpperCase()} playback available`
                                : "Live stream ready for playback"}
                            </p>
                          </div>
                        </div>
                        <Button
                          size="sm"
                          onClick={() => playChannelMutation.mutate(channel.id)}
                          disabled={playChannelMutation.isPending}
                        >
                          <Play data-icon="inline-start" />
                          Play
                        </Button>
                      </div>
                    </div>
                  ))
                ) : (
                  <Empty className="border-0">
                    <EmptyHeader>
                      <EmptyMedia variant="icon">
                        <TvMinimal aria-hidden="true" />
                      </EmptyMedia>
                      <EmptyTitle>No channel matches</EmptyTitle>
                      <EmptyDescription>Try a broader term or switch to the programs tab for EPG hits.</EmptyDescription>
                    </EmptyHeader>
                  </Empty>
                )}
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="programs" className="mt-0">
            <Card>
              <CardHeader>
                <CardTitle>EPG matches</CardTitle>
                <CardDescription>Program results can be live, archived, upcoming, or informational depending on provider availability.</CardDescription>
              </CardHeader>
              <CardContent className="p-0">
                {programs.length ? (
                  programs.map((program, index) => {
                    const playbackState = getProgramPlaybackState(program);

                    return (
                      <div key={program.id}>
                        {index > 0 ? <Separator /> : null}
                        <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                          <div className="flex min-w-0 items-start gap-4">
                            <ChannelAvatar name={program.channelName ?? program.title} className="size-10" />
                            <div className="flex min-w-0 flex-1 flex-col gap-2">
                              <div className="flex flex-wrap items-center gap-2">
                                <h2 className="truncate text-base font-semibold">{program.title}</h2>
                                {program.channelName ? <Badge variant="outline">{program.channelName}</Badge> : null}
                                <ProgramStateBadge state={playbackState} />
                              </div>
                              <p className="text-sm text-muted-foreground">
                                {program.description ?? `Scheduled for ${formatTimeRange(program.startAt, program.endAt)}.`}
                              </p>
                            </div>
                          </div>
                          <div className="flex items-center gap-3 self-start lg:self-center">
                            <Badge variant="outline">{formatTimeRange(program.startAt, program.endAt)}</Badge>
                            {canPlayProgram(program) ? (
                              <Button
                                size="sm"
                                variant="secondary"
                                onClick={() => {
                                  if (playbackState === "live" && program.channelId) {
                                    playChannelMutation.mutate(program.channelId);
                                    return;
                                  }
                                  playProgramMutation.mutate(program.id);
                                }}
                                disabled={playChannelMutation.isPending || playProgramMutation.isPending}
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
                      </div>
                    );
                  })
                ) : (
                  <Empty className="border-0">
                    <EmptyHeader>
                      <EmptyMedia variant="icon">
                        <Clapperboard aria-hidden="true" />
                      </EmptyMedia>
                      <EmptyTitle>No EPG matches</EmptyTitle>
                      <EmptyDescription>Try a title, team, league, or event keyword with synced guide data.</EmptyDescription>
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
