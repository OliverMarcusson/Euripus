import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { OnDemandCategory, OnDemandHistoryEntry, OnDemandMediaType, OnDemandTitle } from "@euripus/shared";
import { Clapperboard, Clock3, Heart, Play, RotateCcw, Search, Tv } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  addOnDemandCategoryFavorite,
  addOnDemandTitleFavorite,
  getOnDemandCategories,
  getOnDemandHistory,
  getOnDemandTitle,
  getOnDemandTitles,
  getSeriesEpisodes,
  removeOnDemandCategoryFavorite,
  removeOnDemandTitleFavorite,
} from "@/lib/api";
import { useOnDemandPlaybackMutation } from "@/hooks/use-playback-actions";
import { cn } from "@/lib/utils";

const PAGE_SIZE = 48;

export function OnDemandPage() {
  const [view, setView] = useState<OnDemandMediaType | "history">("movie");
  const [mediaType, setMediaType] = useState<OnDemandMediaType>("movie");
  const [categoryId, setCategoryId] = useState<string>();
  const [queryInput, setQueryInput] = useState("");
  const [query, setQuery] = useState("");
  const [categoryQuery, setCategoryQuery] = useState("");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<OnDemandTitle | null>(null);
  const categoryFavorite = useCategoryFavoriteMutation();
  const titleFavorite = useTitleFavoriteMutation();

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(queryInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [queryInput]);
  useEffect(() => {
    setCategoryId(undefined);
    setCategoryQuery("");
    setFavoriteOnly(false);
    setOffset(0);
    setSelected(null);
  }, [mediaType]);
  useEffect(() => { setOffset(0); }, [categoryId, query, favoriteOnly]);

  const categoriesQuery = useQuery({
    queryKey: ["on-demand", "categories", mediaType],
    queryFn: () => getOnDemandCategories(mediaType),
    enabled: view !== "history",
  });
  const titlesQuery = useQuery({
    queryKey: ["on-demand", "titles", mediaType, categoryId, query, favoriteOnly, offset],
    queryFn: () => getOnDemandTitles(mediaType, { categoryId, query, favoriteOnly, offset, limit: PAGE_SIZE }),
    enabled: view !== "history",
  });
  const historyQuery = useQuery({
    queryKey: ["on-demand", "history"],
    queryFn: getOnDemandHistory,
    enabled: view === "history",
  });
  const categories = categoriesQuery.data ?? [];
  const visibleCategories = useMemo(() => {
    const term = categoryQuery.trim().toLocaleLowerCase();
    return categories.filter((category) => !term || category.name.toLocaleLowerCase().includes(term));
  }, [categories, categoryQuery]);
  const page = titlesQuery.data;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="On Demand" />
      <Tabs value={view} onValueChange={(value) => {
        const next = value as OnDemandMediaType | "history";
        setView(next);
        if (next !== "history") setMediaType(next);
      }}>
        <TabsList><TabsTrigger value="movie">Movies</TabsTrigger><TabsTrigger value="series">Series</TabsTrigger><TabsTrigger value="history">History</TabsTrigger></TabsList>
      </Tabs>

      {view === "history" ? (
        <HistoryPanel
          entries={historyQuery.data ?? []}
          isPending={historyQuery.isPending}
          isError={historyQuery.isError}
          onOpen={setSelected}
        />
      ) : <>
      <div className="grid gap-3 lg:grid-cols-2">
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-3 size-4 text-muted-foreground" />
          <Input className="pl-9" value={queryInput} onChange={(event) => setQueryInput(event.target.value)} placeholder={`Search ${mediaType === "movie" ? "movies" : "series"}`} />
        </div>
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-3 size-4 text-muted-foreground" />
          <Input className="pl-9" value={categoryQuery} onChange={(event) => setCategoryQuery(event.target.value)} placeholder="Search categories" />
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button size="sm" variant={!categoryId ? "default" : "outline"} onClick={() => setCategoryId(undefined)}>All categories</Button>
        <Button size="sm" variant={favoriteOnly ? "default" : "outline"} onClick={() => setFavoriteOnly((value) => !value)}>
          <Heart className={cn("size-4", favoriteOnly && "fill-current")} /> Favorite titles
        </Button>
      </div>

      {visibleCategories.length ? (
        <div className="flex gap-2 overflow-x-auto pb-1" aria-label="On-demand categories">
          {visibleCategories.map((category) => (
            <div key={category.id} className={cn("flex shrink-0 items-center rounded-md border", categoryId === category.id ? "border-primary bg-primary text-primary-foreground" : "border-input bg-background")}>
              <button className="px-3 py-1.5 text-sm font-medium" onClick={() => setCategoryId(category.id)}>{category.name} <span className="opacity-70">{category.titleCount}</span></button>
              <button
                className="mr-1 rounded p-1 hover:bg-black/10"
                aria-label={`${category.isFavorite ? "Unfavorite" : "Favorite"} category ${category.name}`}
                onClick={() => categoryFavorite.mutate(category)}
              >
                <Heart className={cn("size-4", category.isFavorite && "fill-current")} />
              </button>
            </div>
          ))}
        </div>
      ) : categoryQuery ? <p className="text-sm text-muted-foreground">No matching categories.</p> : null}

      {titlesQuery.isPending ? <Card><CardContent className="p-8 text-muted-foreground">Loading catalog...</CardContent></Card> : null}
      {titlesQuery.isError ? <Card><CardContent className="p-8 text-destructive">Unable to load the on-demand catalog.</CardContent></Card> : null}
      {!titlesQuery.isPending && !titlesQuery.isError && !page?.items.length ? (
        <Empty><EmptyHeader><EmptyMedia variant="icon"><Clapperboard /></EmptyMedia><EmptyTitle>{favoriteOnly ? "No favorite titles yet" : "No on-demand titles found"}</EmptyTitle></EmptyHeader></Empty>
      ) : null}
      {page?.items.length ? (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {page.items.map((title) => <TitleCard key={title.id} title={title} onOpen={() => setSelected(title)} onFavorite={() => titleFavorite.mutate(title)} />)}
        </div>
      ) : null}
      {page ? <div className="flex items-center justify-between"><Button variant="outline" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>Previous</Button><span className="text-sm text-muted-foreground">{page.totalCount ? `${offset + 1}–${Math.min(offset + PAGE_SIZE, page.totalCount)} of ${page.totalCount}` : "0 titles"}</span><Button variant="outline" disabled={page.nextOffset == null} onClick={() => setOffset(page.nextOffset ?? offset)}>Next</Button></div> : null}
      </>}
      <TitleDialog title={selected} onOpenChange={(open) => { if (!open) setSelected(null); }} onFavorite={(title) => titleFavorite.mutate(title)} />
    </div>
  );
}

function HistoryPanel({ entries, isPending, isError, onOpen }: { entries: OnDemandHistoryEntry[]; isPending: boolean; isError: boolean; onOpen: (title: OnDemandTitle) => void }) {
  const moviePlayback = useOnDemandPlaybackMutation("onDemand");
  const episodePlayback = useOnDemandPlaybackMutation("episode");
  if (isPending) return <Card><CardContent className="p-8 text-muted-foreground">Loading history...</CardContent></Card>;
  if (isError) return <Card><CardContent className="p-8 text-destructive">Unable to load playback history.</CardContent></Card>;
  if (!entries.length) return <Empty><EmptyHeader><EmptyMedia variant="icon"><Clock3 /></EmptyMedia><EmptyTitle>No playback history yet</EmptyTitle></EmptyHeader></Empty>;
  return <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{entries.map((entry) => {
    const playback = entry.episodeId ? episodePlayback : moviePlayback;
    const id = entry.episodeId ?? entry.id;
    const progress = entry.durationSeconds ? Math.min(100, (entry.positionSeconds / entry.durationSeconds) * 100) : 0;
    return <Card key={entry.id} className="overflow-hidden"><button className="flex w-full gap-4 p-4 text-left" onClick={() => onOpen(entry)}>
      <div className="h-28 w-20 shrink-0 overflow-hidden rounded-md bg-muted">{entry.posterUrl ? <img src={entry.posterUrl} alt="" className="size-full object-cover" /> : <div className="grid size-full place-items-center"><Tv className="size-7 text-muted-foreground/40" /></div>}</div>
      <div className="min-w-0"><p className="line-clamp-2 font-semibold">{entry.name}</p>{entry.episodeId ? <p className="mt-1 text-sm text-muted-foreground">S{entry.seasonNumber} E{entry.episodeNumber} · {entry.episodeName}</p> : null}<p className="mt-2 text-xs text-muted-foreground">Last played {new Date(entry.lastPlayedAt).toLocaleDateString()}</p></div>
    </button><CardContent className="space-y-3 px-4 pb-4">
      {entry.durationSeconds ? <div><Progress value={progress} className="h-1.5" /><p className="mt-1 text-xs text-muted-foreground">{formatPlaybackTime(entry.positionSeconds)} of {formatPlaybackTime(entry.durationSeconds)}</p></div> : null}
      <div className="flex flex-wrap gap-2"><Button size="sm" variant="outline" onClick={() => playback.mutate({ id, startAtSeconds: 0, resetProgress: true })} disabled={playback.isPending}><RotateCcw />Start over</Button><Button size="sm" onClick={() => playback.mutate({ id, startAtSeconds: entry.positionSeconds })} disabled={playback.isPending}><Play />Continue</Button></div>
    </CardContent></Card>;
  })}</div>;
}

function formatPlaybackTime(seconds: number) {
  const value = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const remaining = value % 60;
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${String(remaining).padStart(2, "0")}` : `${minutes}:${String(remaining).padStart(2, "0")}`;
}

function TitleCard({ title, onOpen, onFavorite }: { title: OnDemandTitle; onOpen: () => void; onFavorite: () => void }) {
  return <div className="group relative overflow-hidden rounded-xl border border-border/50 bg-card transition hover:border-primary/50">
    <button className="block w-full text-left" onClick={onOpen}>
      <div className="aspect-[2/3] bg-muted">{title.posterUrl ? <img src={title.posterUrl} alt="" loading="lazy" className="size-full object-cover transition group-hover:scale-[1.02]" /> : <div className="grid size-full place-items-center"><Tv className="size-10 text-muted-foreground/40" /></div>}</div>
      <div className="p-3"><p className="line-clamp-2 font-medium">{title.name}</p><p className="mt-1 text-xs text-muted-foreground">{title.releaseDate ?? title.categoryName ?? ""}</p></div>
    </button>
    <Button size="icon" variant="secondary" className="absolute right-2 top-2" aria-label={`${title.isFavorite ? "Unfavorite" : "Favorite"} ${title.name}`} onClick={onFavorite}>
      <Heart className={cn("size-4", title.isFavorite && "fill-current")} />
    </Button>
  </div>;
}

function TitleDialog({ title, onOpenChange, onFavorite }: { title: OnDemandTitle | null; onOpenChange: (open: boolean) => void; onFavorite: (title: OnDemandTitle) => void }) {
  const moviePlayback = useOnDemandPlaybackMutation("onDemand");
  const episodePlayback = useOnDemandPlaybackMutation("episode");
  const detailsQuery = useQuery({ queryKey: ["on-demand", "title", title?.id], queryFn: () => getOnDemandTitle(title!.id), enabled: !!title });
  const item = detailsQuery.data ?? title;
  const episodesQuery = useQuery({ queryKey: ["on-demand", "episodes", title?.id], queryFn: () => getSeriesEpisodes(title!.id), enabled: title?.mediaType === "series" });
  const seasons = useMemo(() => [...new Set((episodesQuery.data ?? []).map((episode) => episode.seasonNumber))], [episodesQuery.data]);
  const [season, setSeason] = useState<number>();
  useEffect(() => { setSeason(seasons[0]); }, [title?.id, seasons[0]]);
  return <Dialog open={!!title} onOpenChange={onOpenChange}><DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">{item ? <>
    <DialogHeader><div className="flex items-center justify-between gap-3"><DialogTitle>{item.name}</DialogTitle><Button size="icon" variant="outline" aria-label={`${item.isFavorite ? "Unfavorite" : "Favorite"} ${item.name}`} onClick={() => onFavorite(item)}><Heart className={cn("size-4", item.isFavorite && "fill-current")} /></Button></div></DialogHeader>
    <div className="flex flex-wrap gap-2">{item.genre ? <Badge variant="outline">{item.genre}</Badge> : null}{item.rating != null ? <Badge variant="outline">★ {item.rating}</Badge> : null}{item.durationMinutes ? <Badge variant="outline">{item.durationMinutes} min</Badge> : null}</div>
    {item.plot ? <p className="text-sm leading-6 text-muted-foreground">{item.plot}</p> : null}
    {item.mediaType === "movie" ? <Button onClick={() => moviePlayback.mutate({ id: item.id, startAtSeconds: 0 })} disabled={moviePlayback.isPending}><Play data-icon="inline-start" />Play</Button> : <div className="flex flex-col gap-4">
      {seasons.length > 1 ? <div className="flex flex-wrap gap-2">{seasons.map((value) => <Button key={value} size="sm" variant={season === value ? "default" : "outline"} onClick={() => setSeason(value)}>Season {value}</Button>)}</div> : null}
      {episodesQuery.isPending ? <p className="text-sm text-muted-foreground">Loading episodes...</p> : null}
      {episodesQuery.isError ? <p className="text-sm text-destructive">Unable to load episodes from this provider.</p> : null}
      {(episodesQuery.data ?? []).filter((episode) => episode.seasonNumber === season).map((episode) => <div key={episode.id} className="flex items-start justify-between gap-4 border-t border-border/50 pt-3"><div><p className="font-medium">{episode.episodeNumber}. {episode.name}</p>{episode.plot ? <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{episode.plot}</p> : null}</div><Button size="sm" onClick={() => episodePlayback.mutate({ id: episode.id, startAtSeconds: 0 })} disabled={episodePlayback.isPending}><Play /></Button></div>)}
    </div>}
  </> : null}</DialogContent></Dialog>;
}

function useCategoryFavoriteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (category: OnDemandCategory) => category.isFavorite ? removeOnDemandCategoryFavorite(category.id) : addOnDemandCategoryFavorite(category.id),
    onMutate: async (category) => {
      const key = ["on-demand", "categories", category.mediaType];
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<OnDemandCategory[]>(key);
      queryClient.setQueryData<OnDemandCategory[]>(key, (current) => current?.map((entry) => entry.id === category.id ? { ...entry, isFavorite: !entry.isFavorite } : entry));
      return { key, previous };
    },
    onError: (_error, _category, context) => { if (context) queryClient.setQueryData(context.key, context.previous); },
    onSettled: async (_data, _error, category) => { await queryClient.invalidateQueries({ queryKey: ["on-demand", "categories", category.mediaType] }); },
  });
}

function useTitleFavoriteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (title: OnDemandTitle) => title.isFavorite ? removeOnDemandTitleFavorite(title.id) : addOnDemandTitleFavorite(title.id),
    onMutate: async (title) => {
      await queryClient.cancelQueries({ queryKey: ["on-demand", "titles", title.mediaType] });
      queryClient.setQueriesData<{ items: OnDemandTitle[] }>({ queryKey: ["on-demand", "titles", title.mediaType] }, (current) => current ? { ...current, items: current.items.map((entry) => entry.id === title.id ? { ...entry, isFavorite: !entry.isFavorite } : entry) } : current);
      queryClient.setQueryData<OnDemandTitle>(["on-demand", "title", title.id], (current) => current ? { ...current, isFavorite: !current.isFavorite } : current);
    },
    onSettled: async (_data, _error, title) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["on-demand", "titles", title.mediaType] }),
        queryClient.invalidateQueries({ queryKey: ["on-demand", "title", title.id] }),
      ]);
    },
  });
}
