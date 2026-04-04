import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type { GuideCategorySummary } from "@euripus/shared";
import { ChevronDown, ChevronRight, Heart, Play, RefreshCcw } from "lucide-react";
import { addFavorite, getGuide, getGuideCategory, removeFavorite, startChannelPlayback } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { usePlayerStore } from "@/store/player-store";

const GUIDE_PAGE_SIZE = 40;

type FavoriteMutationPayload = {
  channelId: string;
  favorite: boolean;
};

type GuideCategorySectionProps = {
  category: GuideCategorySummary;
  open: boolean;
  onToggle: () => void;
  onFavorite: (payload: FavoriteMutationPayload) => void;
  onPlay: (channelId: string) => void;
};

export function GuidePage() {
  const queryClient = useQueryClient();
  const [openCategories, setOpenCategories] = useState<string[]>([]);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);
  const guideQuery = useQuery({
    queryKey: ["guide", "overview"],
    queryFn: getGuide,
  });
  const favoriteMutation = useMutation({
    mutationFn: async ({ channelId, favorite }: FavoriteMutationPayload) =>
      favorite ? removeFavorite(channelId) : addFavorite(channelId),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["favorites"] }),
        queryClient.invalidateQueries({ queryKey: ["guide", "overview"] }),
        queryClient.invalidateQueries({ queryKey: ["guide", "category"] }),
      ]);
    },
  });
  const playMutation = useMutation({
    mutationFn: startChannelPlayback,
    onMutate: () => setLoading(true),
    onSuccess: (source) => setSource(source),
    onSettled: () => setLoading(false),
  });

  function toggleCategory(categoryId: string) {
    setOpenCategories((current) =>
      current.includes(categoryId) ? current.filter((id) => id !== categoryId) : [...current, categoryId],
    );
  }

  async function refreshGuide() {
    await queryClient.invalidateQueries({ queryKey: ["guide"] });
  }

  const categories = guideQuery.data?.categories ?? [];

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-semibold">Live Guide</h2>
          <p className="text-sm text-muted-foreground">Current channels with matching EPG programs in the next window.</p>
        </div>
        <Button variant="outline" onClick={refreshGuide}>
          <RefreshCcw />
          Refresh
        </Button>
      </div>
      <div className="grid gap-4">
        {categories.map((category) => (
          <GuideCategorySection
            key={category.id}
            category={category}
            open={openCategories.includes(category.id)}
            onToggle={() => toggleCategory(category.id)}
            onFavorite={(payload) => favoriteMutation.mutate(payload)}
            onPlay={(channelId) => playMutation.mutate(channelId)}
          />
        ))}
      </div>
      {!guideQuery.isLoading && !categories.length ? (
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

function GuideCategorySection({ category, open, onToggle, onFavorite, onPlay }: GuideCategorySectionProps) {
  const categoryQuery = useInfiniteQuery({
    queryKey: ["guide", "category", category.id],
    queryFn: ({ pageParam }) => getGuideCategory(category.id, pageParam, GUIDE_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextOffset ?? undefined,
    enabled: open,
  });
  const entries = categoryQuery.data?.pages.flatMap((page) => page.entries) ?? [];
  const hasEntries = entries.length > 0;
  const isInitialLoading = open && categoryQuery.isLoading && !hasEntries;
  const Icon = open ? ChevronDown : ChevronRight;

  return (
    <Card>
      <CardHeader className="gap-4 md:flex-row md:items-start md:justify-between">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <CardTitle>{category.name}</CardTitle>
            <Badge>{category.channelCount} channels</Badge>
            <Badge>{category.liveNowCount} live now</Badge>
          </div>
          <CardDescription>
            {category.channelCount === 1 ? "1 channel in this guide category." : `${category.channelCount} channels in this guide category.`}
          </CardDescription>
        </div>
        <Button variant="ghost" onClick={onToggle} aria-expanded={open}>
          <Icon />
          {open ? "Hide channels" : "Show channels"}
        </Button>
      </CardHeader>
      {open ? (
        <CardContent className="flex flex-col gap-3 pt-0">
          {isInitialLoading ? <p className="text-sm text-muted-foreground">Loading channels...</p> : null}
          {categoryQuery.isError ? <p className="text-sm text-destructive">Unable to load this category right now.</p> : null}
          {hasEntries ? (
            <>
              {entries.map(({ channel, program }) => (
                <div key={channel.id} className="flex items-center gap-4 rounded-xl border border-border bg-card/70 p-4 max-md:flex-col max-md:items-start">
                  <div className="flex flex-1 items-center gap-4">
                    <div className="flex size-14 items-center justify-center rounded-2xl bg-secondary text-lg font-semibold">
                      {channel.name.slice(0, 2).toUpperCase()}
                    </div>
                    <div className="flex flex-col gap-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="text-lg font-semibold">{channel.name}</h3>
                        {channel.hasCatchup ? <Badge>Catch-up</Badge> : null}
                        {program && isProgramLive(program.startAt, program.endAt) ? <Badge>Live now</Badge> : null}
                      </div>
                      <p className="text-sm text-muted-foreground">{program?.title ?? "No current program metadata synced yet."}</p>
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Button variant="secondary" onClick={() => onFavorite({ channelId: channel.id, favorite: channel.isFavorite })}>
                      <Heart />
                      {channel.isFavorite ? "Unfavorite" : "Favorite"}
                    </Button>
                    <Button onClick={() => onPlay(channel.id)}>
                      <Play />
                      Play
                    </Button>
                  </div>
                </div>
              ))}
              {categoryQuery.hasNextPage ? (
                <Button
                  variant="outline"
                  onClick={() => categoryQuery.fetchNextPage()}
                  disabled={categoryQuery.isFetchingNextPage}
                >
                  {categoryQuery.isFetchingNextPage ? "Loading more..." : "Load more"}
                </Button>
              ) : null}
            </>
          ) : null}
          {!isInitialLoading && !hasEntries && !categoryQuery.isError ? (
            <p className="text-sm text-muted-foreground">No channels with matching EPG data are available in this category.</p>
          ) : null}
        </CardContent>
      ) : null}
    </Card>
  );
}

function isProgramLive(startAt: string, endAt: string) {
  const now = Date.now();
  return new Date(startAt).getTime() <= now && new Date(endAt).getTime() > now;
}
