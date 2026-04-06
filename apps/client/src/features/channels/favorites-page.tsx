import { useQuery } from "@tanstack/react-query";
import { Heart, Play } from "lucide-react";
import type { Program } from "@euripus/shared";
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
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";
import { getFavorites } from "@/lib/api";
import {
  formatArchiveDuration,
  formatTimeRange,
  getProgramPlaybackState,
  getTimeProgress,
  type ProgramPlaybackState,
} from "@/lib/utils";
import { Progress } from "@/components/ui/progress";

export function FavoritesPage() {
  const favoritesQuery = useQuery({
    queryKey: ["favorites"],
    queryFn: getFavorites,
  });
  const favoriteMutation = useChannelFavoriteMutation();
  const playMutation = useChannelPlaybackMutation();

  const favorites = favoritesQuery.data ?? [];

  return (
    <div className="flex flex-col gap-5 sm:gap-6">
      <PageHeader
        title="Favorites"
        meta={<Badge variant="accent">{favorites.length} saved</Badge>}
      />

      {favoritesQuery.isPending ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="flex flex-col gap-3 px-0 pt-0 pb-0 sm:p-5 sm:pt-5">
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
          <CardHeader className="px-0 pt-0 pb-4 sm:p-5 sm:pb-0">
            <CardTitle>Saved channels</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {favorites.map(({ channel, program }, index) => (
              <div key={channel.id} className="group">
                {index > 0 ? <Separator /> : null}
                <div className="flex flex-col gap-4 p-4 sm:gap-5 sm:p-5 transition-colors hover:bg-muted/30">
                  <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-4">
                    <div className="flex min-w-0 flex-1 items-start gap-3 sm:gap-4">
                      <ChannelAvatar
                        name={channel.name}
                        logoUrl={channel.logoUrl}
                        className="h-12 w-12 shrink-0 rounded-xl ring-1 ring-border/10 sm:h-14 sm:w-14 sm:rounded-2xl"
                        fallbackClassName="rounded-xl sm:rounded-2xl"
                      />
                      <div className="flex min-w-0 flex-1 flex-col gap-1.5 pt-0.5">
                        <div className="flex min-w-0 flex-wrap items-center gap-2">
                          <h2 className="min-w-0 break-words text-base font-semibold tracking-tight sm:text-lg">
                            {channel.name}
                          </h2>
                          {channel.streamExtension ? (
                            <Badge variant="outline" className="bg-background/50 border-transparent text-[10px] uppercase">
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
            ))}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}

function FavoriteProgramDetails({
  program,
}: {
  program: Program;
}) {
  const playbackState = getProgramPlaybackState(program);
  const isLive = playbackState === "live";

  return (
    <div className="flex min-w-0 flex-col gap-2">
      <div className="flex flex-wrap items-start gap-x-2 gap-y-1">
        <p className="min-w-0 break-words text-sm font-semibold leading-6">
          {program.title}
        </p>
        <ProgramStateBadge state={playbackState} />
      </div>
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        <span className="font-medium text-foreground/80">
          {formatTimeRange(program.startAt, program.endAt)}
        </span>
        {program.canCatchup ? (
          <Badge variant="outline" className="text-[10px] h-5 px-1.5 py-0 uppercase tracking-widest font-medium opacity-80">
            Catch-up window
          </Badge>
        ) : null}
      </div>
      {program.description ? (
        <p className="line-clamp-2 max-w-4xl text-sm leading-relaxed text-muted-foreground">
          {program.description}
        </p>
      ) : null}
      {isLive ? (
        <Progress
          value={getTimeProgress(program.startAt, program.endAt)}
          className="mt-2 h-1.5 bg-border/50"
        />
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
