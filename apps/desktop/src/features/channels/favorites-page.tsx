import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Heart, HeartOff, Play } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { getFavorites, removeFavorite, startChannelPlayback } from "@/lib/api";
import { formatArchiveDuration } from "@/lib/utils";
import { usePlayerStore } from "@/store/player-store";

export function FavoritesPage() {
  const queryClient = useQueryClient();
  const favoritesQuery = useQuery({ queryKey: ["favorites"], queryFn: getFavorites });
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const removeMutation = useMutation({
    mutationFn: removeFavorite,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
      ]);
    },
  });
  const playMutation = useMutation({
    mutationFn: startChannelPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });

  const favorites = favoritesQuery.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Favorites"
        description="Server-backed favorites follow your account, giving you a personal shortlist of channels to jump into quickly."
        meta={<Badge variant="accent">{favorites.length} saved</Badge>}
      />

      {favoritesQuery.isPending ? (
        <Card>
          <CardContent className="flex flex-col gap-3 pt-5">
            {Array.from({ length: 4 }).map((_, index) => (
              <div key={index} className="flex items-center gap-4 rounded-xl border border-border/70 p-4">
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
        <Card>
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Heart aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>No favorites yet</EmptyTitle>
                <EmptyDescription>
                  Favorite channels from the guide to build a faster start screen for live viewing.
                </EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!favoritesQuery.isPending && favorites.length ? (
        <Card>
          <CardHeader>
            <CardTitle>Saved channels</CardTitle>
            <CardDescription>These channels are pinned to your account and stay in sync across devices.</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            {favorites.map((channel, index) => (
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
                        {channel.archiveDurationHours ? (
                          <Badge>{formatArchiveDuration(channel.archiveDurationHours)}</Badge>
                        ) : null}
                      </div>
                      <p className="text-sm text-muted-foreground">
                        {channel.streamExtension
                          ? `Primary stream format: ${channel.streamExtension.toUpperCase()}`
                          : "Live stream ready for playback."}
                      </p>
                    </div>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => removeMutation.mutate(channel.id)}
                      disabled={removeMutation.isPending}
                    >
                      <HeartOff data-icon="inline-start" />
                      Remove
                    </Button>
                    <Button size="sm" onClick={() => playMutation.mutate(channel.id)} disabled={playMutation.isPending}>
                      <Play data-icon="inline-start" />
                      Play
                    </Button>
                  </div>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
