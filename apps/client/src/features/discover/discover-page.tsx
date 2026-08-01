import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type {
  DiscoverChart,
  DiscoverCountryMode,
  DiscoverTitle,
  OnDemandMediaType,
} from "@euripus/shared";
import { Compass, Library, Star, Tv } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getDiscoverCharts, getDiscoverTitles } from "@/lib/api";
import { TitleDialog } from "@/features/on-demand/title-dialog";
import { cn } from "@/lib/utils";

const PAGE_SIZE = 40;

const CHART_LABELS: Record<DiscoverChart, string> = {
  trending: "Trending",
  popular: "Popular",
  top_rated: "Top rated",
};

const COUNTRY_MODE_LABELS: Record<DiscoverCountryMode, string> = {
  global: "Worldwide",
  available_in: "Available in",
  from: "From",
};

const countryNames =
  typeof Intl.DisplayNames === "function"
    ? new Intl.DisplayNames(["en"], { type: "region" })
    : null;

function countryLabel(code: string | undefined) {
  // `of` throws a RangeError on anything that is not a well-formed region code, and there
  // is a render between picking a country mode and the default country being applied where
  // this is called with nothing.
  if (!code) return "";
  try {
    return countryNames?.of(code) ?? code;
  } catch {
    return code;
  }
}

export function DiscoverPage() {
  const [mediaType, setMediaType] = useState<OnDemandMediaType>("movie");
  const [chart, setChart] = useState<DiscoverChart>("trending");
  const [countryMode, setCountryMode] = useState<DiscoverCountryMode>("global");
  const [country, setCountry] = useState<string>();
  const [availableOnly, setAvailableOnly] = useState(false);
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<DiscoverTitle | null>(null);

  const chartsQuery = useQuery({
    queryKey: ["discover", "charts"],
    queryFn: getDiscoverCharts,
  });
  const config = chartsQuery.data;

  const modes = useMemo(() => {
    const seen = new Set<DiscoverCountryMode>();
    for (const option of config?.charts ?? []) seen.add(option.countryMode);
    return [...seen];
  }, [config]);

  // TMDB cannot serve every chart in every country mode (there is no country-scoped
  // trending chart, for one), so the chart buttons follow the selected mode rather than a
  // hardcoded list.
  const chartsForMode = useMemo(
    () =>
      (config?.charts ?? [])
        .filter((option) => option.countryMode === countryMode)
        .map((option) => option.chart),
    [config, countryMode],
  );

  useEffect(() => {
    if (!config) return;
    if (countryMode !== "global" && !country) setCountry(config.countries[0]);
  }, [config, countryMode, country]);

  useEffect(() => {
    if (chartsForMode.length && !chartsForMode.includes(chart)) setChart(chartsForMode[0]);
  }, [chartsForMode, chart]);

  useEffect(() => { setOffset(0); }, [mediaType, chart, countryMode, country, availableOnly]);

  const ready = countryMode === "global" || !!country;
  const titlesQuery = useQuery({
    queryKey: ["discover", "titles", mediaType, chart, countryMode, country, availableOnly, offset],
    queryFn: () =>
      getDiscoverTitles(mediaType, { chart, countryMode, country, availableOnly, offset, limit: PAGE_SIZE }),
    enabled: !!config?.enabled && ready && chartsForMode.includes(chart),
  });
  const page = titlesQuery.data;

  if (chartsQuery.isPending) {
    return <div className="flex flex-col gap-6"><PageHeader title="Discover" /><Card><CardContent className="p-8 text-muted-foreground">Loading charts...</CardContent></Card></div>;
  }
  if (config && !config.enabled) {
    return <div className="flex flex-col gap-6">
      <PageHeader title="Discover" />
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon"><Compass /></EmptyMedia>
          <EmptyTitle>Discover is not configured</EmptyTitle>
        </EmptyHeader>
        <p className="text-sm text-muted-foreground">Set APP_TMDB_API_KEY on the server to browse TMDB charts.</p>
      </Empty>
    </div>;
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Discover" />

      <Tabs value={mediaType} onValueChange={(value) => setMediaType(value as OnDemandMediaType)}>
        <TabsList><TabsTrigger value="movie">Movies</TabsTrigger><TabsTrigger value="series">Series</TabsTrigger></TabsList>
      </Tabs>

      <div className="flex flex-wrap gap-2" aria-label="Country scope">
        {modes.map((mode) => (
          <Button key={mode} size="sm" variant={countryMode === mode ? "default" : "outline"} onClick={() => setCountryMode(mode)}>
            {COUNTRY_MODE_LABELS[mode]}
          </Button>
        ))}
        {countryMode !== "global" ? (
          <select
            aria-label="Country"
            className="h-8 rounded-md border border-input bg-background px-2 text-sm"
            value={country ?? ""}
            onChange={(event) => setCountry(event.target.value)}
          >
            {(config?.countries ?? []).map((code) => (
              <option key={code} value={code}>{countryLabel(code)}</option>
            ))}
          </select>
        ) : null}
      </div>

      <div className="flex flex-wrap gap-2" aria-label="Charts">
        {chartsForMode.map((option) => (
          <Button key={option} size="sm" variant={chart === option ? "default" : "outline"} onClick={() => setChart(option)}>
            {CHART_LABELS[option]}
          </Button>
        ))}
        <Button
          size="sm"
          variant={availableOnly ? "default" : "outline"}
          aria-pressed={availableOnly}
          onClick={() => setAvailableOnly((value) => !value)}
        >
          <Library className="size-4" /> Only in my providers
        </Button>
      </div>

      {/*
        TMDB has no per-country popularity, so neither country mode is a "most watched
        here" chart. Saying so beats letting the ranking imply something it does not mean.
      */}
      {country && countryMode === "available_in" ? (
        <p className="text-sm text-muted-foreground">Globally popular titles licensed to stream in {countryLabel(country)}.</p>
      ) : null}
      {country && countryMode === "from" ? (
        <p className="text-sm text-muted-foreground">Titles produced in {countryLabel(country)}.</p>
      ) : null}

      {titlesQuery.isPending ? <Card><CardContent className="p-8 text-muted-foreground">Loading titles...</CardContent></Card> : null}
      {titlesQuery.isError ? <Card><CardContent className="p-8 text-destructive">Unable to load Discover charts.</CardContent></Card> : null}
      {!titlesQuery.isPending && !titlesQuery.isError && !page?.items.length ? (
        <Empty><EmptyHeader><EmptyMedia variant="icon"><Compass /></EmptyMedia><EmptyTitle>{availableOnly ? "Nothing on this chart is in your providers" : "This chart has not been fetched yet"}</EmptyTitle></EmptyHeader></Empty>
      ) : null}

      {page?.items.length ? <>
        {availableOnly ? null : (
          <p className="text-sm text-muted-foreground">{page.availableCount} of {page.items.length} on this page are in your providers.</p>
        )}
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {page.items.map((title) => (
            <DiscoverCard key={`${title.mediaType}-${title.tmdbId}`} title={title} onOpen={() => setSelected(title)} />
          ))}
        </div>
      </> : null}

      {page ? (
        <div className="flex items-center justify-between">
          <Button variant="outline" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>Previous</Button>
          <span className="text-sm text-muted-foreground">{page.totalCount ? `${offset + 1}–${Math.min(offset + PAGE_SIZE, page.totalCount)} of ${page.totalCount}` : "0 titles"}</span>
          <Button variant="outline" disabled={page.nextOffset == null} onClick={() => setOffset(page.nextOffset ?? offset)}>Next</Button>
        </div>
      ) : null}

      <p className="text-xs text-muted-foreground">
        Chart data from The Movie Database (TMDB). This product uses the TMDB API but is not endorsed or certified by TMDB.
      </p>

      <TitleDialog
        titleId={selected?.onDemandTitleId ?? null}
        mediaType={selected?.mediaType ?? null}
        onOpenChange={(open) => { if (!open) setSelected(null); }}
      />
    </div>
  );
}

function DiscoverCard({ title, onOpen }: { title: DiscoverTitle; onOpen: () => void }) {
  const available = title.onDemandTitleId != null;
  return (
    <div className={cn(
      "group relative overflow-hidden rounded-xl border border-border/50 bg-card transition",
      available ? "hover:border-primary/50" : "opacity-60",
    )}>
      <button
        className="block w-full text-left disabled:cursor-default"
        onClick={onOpen}
        disabled={!available}
        aria-label={available ? `Open ${title.title}` : `${title.title} is not available from your providers`}
      >
        <div className="aspect-[2/3] bg-muted">
          {title.posterUrl
            ? <img src={title.posterUrl} alt="" loading="lazy" className={cn("size-full object-cover transition", available && "group-hover:scale-[1.02]")} />
            : <div className="grid size-full place-items-center"><Tv className="size-10 text-muted-foreground/40" /></div>}
        </div>
        <div className="p-3">
          <p className="line-clamp-2 font-medium">{title.title}</p>
          <div className="mt-1 flex items-baseline justify-between gap-2 text-xs text-muted-foreground">
            <span className="truncate">{title.releaseYear ?? ""}</span>
            {title.voteAverage != null ? (
              <span className="flex shrink-0 items-center gap-1"><Star className="size-3" />{title.voteAverage.toFixed(1)}</span>
            ) : null}
          </div>
          <div className="mt-2">
            {available
              ? <Badge variant="accent" className="max-w-full truncate">{title.providerLabel}</Badge>
              : <Badge variant="outline">Not in your providers</Badge>}
          </div>
        </div>
      </button>
      <Badge variant="default" className="absolute left-2 top-2 tabular-nums">#{title.rank}</Badge>
    </div>
  );
}
