import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { OnDemandMediaType, OnDemandTitle } from "@euripus/shared";
import { Clapperboard, Play, Search, Tv } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getOnDemandCategories, getOnDemandTitle, getOnDemandTitles, getSeriesEpisodes } from "@/lib/api";
import { useOnDemandPlaybackMutation } from "@/hooks/use-playback-actions";

const PAGE_SIZE = 48;

export function OnDemandPage() {
  const [mediaType, setMediaType] = useState<OnDemandMediaType>("movie");
  const [categoryId, setCategoryId] = useState<string>();
  const [queryInput, setQueryInput] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<OnDemandTitle | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(queryInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [queryInput]);
  useEffect(() => { setCategoryId(undefined); setOffset(0); setSelected(null); }, [mediaType]);
  useEffect(() => { setOffset(0); }, [categoryId, query]);

  const categoriesQuery = useQuery({
    queryKey: ["on-demand", "categories", mediaType],
    queryFn: () => getOnDemandCategories(mediaType),
  });
  const titlesQuery = useQuery({
    queryKey: ["on-demand", "titles", mediaType, categoryId, query, offset],
    queryFn: () => getOnDemandTitles(mediaType, { categoryId, query, offset, limit: PAGE_SIZE }),
  });
  const categories = categoriesQuery.data ?? [];
  const page = titlesQuery.data;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="On Demand" />
      <Tabs value={mediaType} onValueChange={(value) => setMediaType(value as OnDemandMediaType)}>
        <TabsList><TabsTrigger value="movie">Movies</TabsTrigger><TabsTrigger value="series">Series</TabsTrigger></TabsList>
      </Tabs>
      <div className="relative max-w-xl">
        <Search className="pointer-events-none absolute left-3 top-3 size-4 text-muted-foreground" />
        <Input className="pl-9" value={queryInput} onChange={(event) => setQueryInput(event.target.value)} placeholder={`Search ${mediaType === "movie" ? "movies" : "series"}`} />
      </div>
      {categories.length ? (
        <div className="flex gap-2 overflow-x-auto pb-1">
          <Button size="sm" variant={!categoryId ? "default" : "outline"} onClick={() => setCategoryId(undefined)}>All</Button>
          {categories.map((category) => <Button key={category.id} size="sm" variant={categoryId === category.id ? "default" : "outline"} onClick={() => setCategoryId(category.id)}>{category.name}<Badge variant="outline">{category.titleCount}</Badge></Button>)}
        </div>
      ) : null}
      {titlesQuery.isPending ? <Card><CardContent className="p-8 text-muted-foreground">Loading catalog...</CardContent></Card> : null}
      {titlesQuery.isError ? <Card><CardContent className="p-8 text-destructive">Unable to load the on-demand catalog.</CardContent></Card> : null}
      {!titlesQuery.isPending && !titlesQuery.isError && !page?.items.length ? (
        <Empty><EmptyHeader><EmptyMedia variant="icon"><Clapperboard /></EmptyMedia><EmptyTitle>No on-demand titles found</EmptyTitle></EmptyHeader></Empty>
      ) : null}
      {page?.items.length ? (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {page.items.map((title) => <TitleCard key={title.id} title={title} onOpen={() => setSelected(title)} />)}
        </div>
      ) : null}
      {page ? <div className="flex items-center justify-between"><Button variant="outline" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>Previous</Button><span className="text-sm text-muted-foreground">{page.totalCount ? `${offset + 1}–${Math.min(offset + PAGE_SIZE, page.totalCount)} of ${page.totalCount}` : "0 titles"}</span><Button variant="outline" disabled={page.nextOffset == null} onClick={() => setOffset(page.nextOffset ?? offset)}>Next</Button></div> : null}
      <TitleDialog title={selected} onOpenChange={(open) => { if (!open) setSelected(null); }} />
    </div>
  );
}

function TitleCard({ title, onOpen }: { title: OnDemandTitle; onOpen: () => void }) {
  return <button className="group overflow-hidden rounded-xl border border-border/50 bg-card text-left transition hover:border-primary/50" onClick={onOpen}>
    <div className="aspect-[2/3] bg-muted">{title.posterUrl ? <img src={title.posterUrl} alt="" loading="lazy" className="size-full object-cover transition group-hover:scale-[1.02]" /> : <div className="grid size-full place-items-center"><Tv className="size-10 text-muted-foreground/40" /></div>}</div>
    <div className="p-3"><p className="line-clamp-2 font-medium">{title.name}</p><p className="mt-1 text-xs text-muted-foreground">{title.releaseDate ?? title.categoryName ?? ""}</p></div>
  </button>;
}

function TitleDialog({ title, onOpenChange }: { title: OnDemandTitle | null; onOpenChange: (open: boolean) => void }) {
  const moviePlayback = useOnDemandPlaybackMutation("onDemand");
  const episodePlayback = useOnDemandPlaybackMutation("episode");
  const detailsQuery = useQuery({ queryKey: ["on-demand", "title", title?.id], queryFn: () => getOnDemandTitle(title!.id), enabled: !!title });
  const item = detailsQuery.data ?? title;
  const episodesQuery = useQuery({ queryKey: ["on-demand", "episodes", title?.id], queryFn: () => getSeriesEpisodes(title!.id), enabled: title?.mediaType === "series" });
  const seasons = useMemo(() => [...new Set((episodesQuery.data ?? []).map((episode) => episode.seasonNumber))], [episodesQuery.data]);
  const [season, setSeason] = useState<number>();
  useEffect(() => { setSeason(seasons[0]); }, [title?.id, seasons[0]]);
  return <Dialog open={!!title} onOpenChange={onOpenChange}><DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">{item ? <>
    <DialogHeader><DialogTitle>{item.name}</DialogTitle></DialogHeader>
    <div className="flex flex-wrap gap-2">{item.genre ? <Badge variant="outline">{item.genre}</Badge> : null}{item.rating != null ? <Badge variant="outline">★ {item.rating}</Badge> : null}{item.durationMinutes ? <Badge variant="outline">{item.durationMinutes} min</Badge> : null}</div>
    {item.plot ? <p className="text-sm leading-6 text-muted-foreground">{item.plot}</p> : null}
    {item.mediaType === "movie" ? <Button onClick={() => moviePlayback.mutate(item.id)} disabled={moviePlayback.isPending}><Play data-icon="inline-start" />Play</Button> : <div className="flex flex-col gap-4">
      {seasons.length > 1 ? <div className="flex flex-wrap gap-2">{seasons.map((value) => <Button key={value} size="sm" variant={season === value ? "default" : "outline"} onClick={() => setSeason(value)}>Season {value}</Button>)}</div> : null}
      {episodesQuery.isPending ? <p className="text-sm text-muted-foreground">Loading episodes...</p> : null}
      {episodesQuery.isError ? <p className="text-sm text-destructive">Unable to load episodes from this provider.</p> : null}
      {(episodesQuery.data ?? []).filter((episode) => episode.seasonNumber === season).map((episode) => <div key={episode.id} className="flex items-start justify-between gap-4 border-t border-border/50 pt-3"><div><p className="font-medium">{episode.episodeNumber}. {episode.name}</p>{episode.plot ? <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{episode.plot}</p> : null}</div><Button size="sm" onClick={() => episodePlayback.mutate(episode.id)} disabled={episodePlayback.isPending}><Play /></Button></div>)}
    </div>}
  </> : null}</DialogContent></Dialog>;
}
