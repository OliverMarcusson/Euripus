import { useMutation, useQuery } from "@tanstack/react-query";
import { useDeferredValue, useState } from "react";
import { Play, Search as SearchIcon } from "lucide-react";
import { searchCatalog, startChannelPlayback, startProgramPlayback } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { usePlayerStore } from "@/store/player-store";

export function SearchPage() {
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const searchQuery = useQuery({
    queryKey: ["search", deferredQuery],
    queryFn: () => searchCatalog(deferredQuery),
    enabled: deferredQuery.trim().length > 1,
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

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h2 className="text-2xl font-semibold">Master Search</h2>
        <p className="text-sm text-muted-foreground">Search all available channels and synced EPG program titles from one input.</p>
      </div>
      <div className="relative">
        <SearchIcon className="pointer-events-none absolute left-3 top-3.5 text-muted-foreground" />
        <Input className="pl-10" placeholder="Search channels, shows, events, teams..." value={query} onChange={(event) => setQuery(event.target.value)} />
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Channels</CardTitle>
            <CardDescription>Fast matches on names and categories.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {(searchQuery.data?.channels ?? []).map((channel) => (
              <div key={channel.id} className="flex items-center justify-between gap-3 rounded-xl border border-border bg-card/70 p-4">
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="font-medium">{channel.name}</p>
                    {channel.categoryName ? <Badge>{channel.categoryName}</Badge> : null}
                  </div>
                  <p className="text-sm text-muted-foreground">{channel.hasCatchup ? "Catch-up available" : "Live stream"}</p>
                </div>
                <Button size="sm" onClick={() => playChannelMutation.mutate(channel.id)}>
                  <Play />
                  Play
                </Button>
              </div>
            ))}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Programs</CardTitle>
            <CardDescription>EPG titles with instant catch-up launch when available.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {(searchQuery.data?.programs ?? []).map((program) => (
              <div key={program.id} className="flex items-center justify-between gap-3 rounded-xl border border-border bg-card/70 p-4">
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="font-medium">{program.title}</p>
                    {program.channelName ? <Badge>{program.channelName}</Badge> : null}
                  </div>
                  <p className="text-sm text-muted-foreground">{program.description ?? "No description available"}</p>
                </div>
                <Button size="sm" variant="secondary" onClick={() => playProgramMutation.mutate(program.id)}>
                  <Play />
                  Play
                </Button>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

