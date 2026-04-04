import { useMutation, useQuery } from "@tanstack/react-query";
import { Heart, Play } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { getFavorites, startChannelPlayback } from "@/lib/api";
import { formatArchiveDuration } from "@/lib/utils";
import { usePlayerStore } from "@/store/player-store";

export function FavoritesPage() {
  const favoritesQuery = useQuery({ queryKey: ["favorites"], queryFn: getFavorites });
  const favoriteMutation = useChannelFavoriteMutation();
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const playMutation = useMutation({
    mutationFn: startChannelPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });

  const favorites = favoritesQuery.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Favorites" meta={<Badge variant="accent">{favorites.length} saved</Badge>} />

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
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!favoritesQuery.isPending && favorites.length ? (
        <Card>
          <CardHeader>
            <CardTitle>Saved channels</CardTitle>
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
                      onClick={() => favoriteMutation.mutate(channel)}
                      disabled={favoriteMutation.isPending && favoriteMutation.variables?.id === channel.id}
                    >
                      <Heart data-icon="inline-start" />
                      Unfavorite
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
