import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Heart, Play, RefreshCcw } from "lucide-react";
import { addFavorite, getGuide, removeFavorite, startChannelPlayback } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { usePlayerStore } from "@/store/player-store";

export function GuidePage() {
  const queryClient = useQueryClient();
  const guideQuery = useQuery({ queryKey: ["guide"], queryFn: getGuide });
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const favoriteMutation = useMutation({
    mutationFn: async ({ channelId, favorite }: { channelId: string; favorite: boolean }) =>
      favorite ? removeFavorite(channelId) : addFavorite(channelId),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
      ]);
    },
  });
  const playMutation = useMutation({
    mutationFn: startChannelPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-semibold">Live Guide</h2>
          <p className="text-sm text-muted-foreground">Current channels with matching EPG programs in the next window.</p>
        </div>
        <Button variant="outline" onClick={() => queryClient.invalidateQueries({ queryKey: ["guide"] })}>
          <RefreshCcw />
          Refresh
        </Button>
      </div>
      <div className="grid gap-4">
        {(guideQuery.data?.channels ?? []).map((channel) => {
          const program = guideQuery.data?.programs.find((item) => item.channelId === channel.id);
          return (
            <Card key={channel.id}>
              <CardContent className="flex items-center gap-4 py-5 max-md:flex-col max-md:items-start">
                <div className="flex flex-1 items-center gap-4">
                  <div className="flex size-14 items-center justify-center rounded-2xl bg-secondary text-lg font-semibold">
                    {channel.name.slice(0, 2).toUpperCase()}
                  </div>
                  <div className="flex flex-col gap-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="text-lg font-semibold">{channel.name}</h3>
                      {channel.categoryName ? <Badge>{channel.categoryName}</Badge> : null}
                      {channel.hasCatchup ? <Badge>Catch-up</Badge> : null}
                    </div>
                    <p className="text-sm text-muted-foreground">{program?.title ?? "No current program metadata synced yet."}</p>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button variant="secondary" onClick={() => favoriteMutation.mutate({ channelId: channel.id, favorite: channel.isFavorite })}>
                    <Heart />
                    {channel.isFavorite ? "Unfavorite" : "Favorite"}
                  </Button>
                  <Button onClick={() => playMutation.mutate(channel.id)}>
                    <Play />
                    Play
                  </Button>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
      {!guideQuery.data?.channels.length ? (
        <Card>
          <CardHeader>
            <CardTitle>No guide data yet</CardTitle>
            <CardDescription>Connect a provider and run a sync to populate channels and EPG results.</CardDescription>
          </CardHeader>
        </Card>
      ) : null}
    </div>
  );
}

