import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { OnDemandMediaType, OnDemandTitle } from "@euripus/shared";
import { Heart, Play } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { getOnDemandTitle, getSeriesEpisodes } from "@/lib/api";
import { useOnDemandPlaybackMutation } from "@/hooks/use-playback-actions";
import { cn } from "@/lib/utils";

/**
 * Detail sheet and playback entry point for a catalog title.
 *
 * Keyed by id rather than by a full title object so Discover can open it from a chart row,
 * where all that is known is the matched on-demand id.
 */
export function TitleDialog({
  titleId,
  mediaType,
  fallback,
  onOpenChange,
  onFavorite,
}: {
  titleId: string | null;
  mediaType: OnDemandMediaType | null;
  /** Rendered until the detail request resolves, avoiding an empty flash. */
  fallback?: OnDemandTitle | null;
  onOpenChange: (open: boolean) => void;
  onFavorite?: (title: OnDemandTitle) => void;
}) {
  const moviePlayback = useOnDemandPlaybackMutation("onDemand");
  const episodePlayback = useOnDemandPlaybackMutation("episode");
  const detailsQuery = useQuery({
    queryKey: ["on-demand", "title", titleId],
    queryFn: () => getOnDemandTitle(titleId!),
    enabled: !!titleId,
  });
  const item = detailsQuery.data ?? fallback ?? null;
  const episodesQuery = useQuery({
    queryKey: ["on-demand", "episodes", titleId],
    queryFn: () => getSeriesEpisodes(titleId!),
    enabled: !!titleId && mediaType === "series",
  });
  const seasons = useMemo(
    () => [...new Set((episodesQuery.data ?? []).map((episode) => episode.seasonNumber))],
    [episodesQuery.data],
  );
  const [season, setSeason] = useState<number>();
  useEffect(() => { setSeason(seasons[0]); }, [titleId, seasons[0]]);

  return <Dialog open={!!titleId} onOpenChange={onOpenChange}><DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">{item ? <>
    <DialogHeader><div className="flex items-center justify-between gap-3"><DialogTitle>{item.name}</DialogTitle>{onFavorite ? <Button size="icon" variant="outline" aria-label={`${item.isFavorite ? "Unfavorite" : "Favorite"} ${item.name}`} onClick={() => onFavorite(item)}><Heart className={cn("size-4", item.isFavorite && "fill-current")} /></Button> : null}</div></DialogHeader>
    <div className="flex flex-wrap gap-2"><Badge variant="accent">{item.providerLabel}</Badge>{item.genre ? <Badge variant="outline">{item.genre}</Badge> : null}{item.rating != null ? <Badge variant="outline">★ {item.rating}</Badge> : null}{item.durationMinutes ? <Badge variant="outline">{item.durationMinutes} min</Badge> : null}</div>
    {item.plot ? <p className="text-sm leading-6 text-muted-foreground">{item.plot}</p> : null}
    {item.mediaType === "movie" ? <Button onClick={() => moviePlayback.mutate({ id: item.id, startAtSeconds: 0 })} disabled={moviePlayback.isPending}><Play data-icon="inline-start" />Play</Button> : <div className="flex flex-col gap-4">
      {seasons.length > 1 ? <div className="flex flex-wrap gap-2">{seasons.map((value) => <Button key={value} size="sm" variant={season === value ? "default" : "outline"} onClick={() => setSeason(value)}>Season {value}</Button>)}</div> : null}
      {episodesQuery.isPending ? <p className="text-sm text-muted-foreground">Loading episodes...</p> : null}
      {episodesQuery.isError ? <p className="text-sm text-destructive">Unable to load episodes from this provider.</p> : null}
      {(episodesQuery.data ?? []).filter((episode) => episode.seasonNumber === season).map((episode) => <div key={episode.id} className="flex items-start justify-between gap-4 border-t border-border/50 pt-3"><div><p className="font-medium">{episode.episodeNumber}. {episode.name}</p>{episode.plot ? <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{episode.plot}</p> : null}</div><Button size="sm" onClick={() => episodePlayback.mutate({ id: episode.id, startAtSeconds: 0 })} disabled={episodePlayback.isPending}><Play /></Button></div>)}
    </div>}
  </> : null}</DialogContent></Dialog>;
}
