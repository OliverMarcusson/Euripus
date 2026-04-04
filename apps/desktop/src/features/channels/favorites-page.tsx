import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { HeartOff, Play } from "lucide-react";
import { getFavorites, removeFavorite, startChannelPlayback } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h2 className="text-2xl font-semibold">Favorites</h2>
        <p className="text-sm text-muted-foreground">Server-backed favorites follow the signed-in account across devices.</p>
      </div>
      {!favoritesQuery.data?.length ? (
        <Card>
          <CardHeader>
            <CardTitle>No favorites yet</CardTitle>
            <CardDescription>Favorite channels from the guide to keep them here.</CardDescription>
          </CardHeader>
        </Card>
      ) : null}
      <div className="grid gap-4">
        {(favoritesQuery.data ?? []).map((channel) => (
          <Card key={channel.id}>
            <CardContent className="flex items-center justify-between gap-4 py-5 max-md:flex-col max-md:items-start">
              <div>
                <h3 className="text-lg font-semibold">{channel.name}</h3>
                <p className="text-sm text-muted-foreground">{channel.categoryName ?? "Uncategorized"}</p>
              </div>
              <div className="flex gap-2">
                <Button variant="secondary" onClick={() => removeMutation.mutate(channel.id)}>
                  <HeartOff />
                  Remove
                </Button>
                <Button onClick={() => playMutation.mutate(channel.id)}>
                  <Play />
                  Play
                </Button>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}

