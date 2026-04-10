import { Radio, RefreshCcw, SlidersHorizontal } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import {
  GuideCategoryFilterCard,
  GuideCategorySection,
} from "@/features/channels/guide-page-sections";
import { useGuidePageState } from "@/features/channels/use-guide-page-state";
import { useCategoryFavoriteMutation } from "@/hooks/use-category-favorite";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";

export function GuidePage() {
  const guideState = useGuidePageState();
  const favoriteMutation = useChannelFavoriteMutation();
  const categoryFavoriteMutation = useCategoryFavoriteMutation();
  const playMutation = useChannelPlaybackMutation();

  return (
    <div className="flex flex-col gap-5 sm:gap-6">
      <PageHeader
        title="Live Guide"
        actions={(
          <Button variant="outline" onClick={guideState.refreshGuide}>
            <RefreshCcw data-icon="inline-start" />
            Refresh
          </Button>
        )}
        meta={(
          <>
            <Badge variant="accent">
              {guideState.categories.length} categories
            </Badge>
            <Badge variant="outline">{guideState.liveCount} live now</Badge>
            <Badge variant="outline">
              {guideState.shouldApplyFilter
                ? `${guideState.visibleCategories.length} matching`
                : guideState.validSelectedCategoryIds.length
                  ? `${guideState.validSelectedCategoryIds.length} included`
                  : "Showing all"}
            </Badge>
          </>
        )}
      />

      {!guideState.guideQuery.isPending && guideState.categories.length ? (
        <GuideCategoryFilterCard
          categories={guideState.chooserCategories}
          filterInput={guideState.filterInput}
          appliedFilter={guideState.appliedFilter}
          preferencesReady={!guideState.preferencesQuery.isPending}
          saving={guideState.savePreferencesMutation.isPending}
          selectedCategoryIds={guideState.validSelectedCategoryIds}
          showOnlyChannelsWithEpg={guideState.showOnlyChannelsWithEpg}
          onFilterInputChange={guideState.setFilterInput}
          onApplyFilter={guideState.applyFilter}
          onReset={() => guideState.updateIncludedCategoryIds([])}
          onToggleEpgOnly={guideState.setShowOnlyChannelsWithEpg}
          onToggleCategory={guideState.toggleIncludedCategory}
        />
      ) : null}

      {guideState.guideQuery.isPending ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="flex flex-col gap-3 px-0 pt-0 pb-0 sm:p-5 sm:pt-5">
            {Array.from({ length: 5 }).map((_, index) => (
              <div
                key={index}
                className="rounded-2xl border border-border/70 p-4 sm:p-5"
              >
                <Skeleton className="h-5 w-40" />
                <Skeleton className="mt-3 h-4 w-24" />
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}

      {!guideState.guideQuery.isPending && !guideState.categories.length ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Radio aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>No guide data yet</EmptyTitle>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!guideState.guideQuery.isPending && guideState.categories.length ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardContent className="p-0">
            {guideState.visibleCategories.length ? (
              guideState.visibleCategories.map((category, index) => (
                <div key={category.id}>
                  {index > 0 ? <Separator /> : null}
                  <GuideCategorySection
                    category={category}
                    open={guideState.openCategories.includes(category.id)}
                    favoritePending={favoriteMutation.isPending}
                    activeFavoriteChannelId={favoriteMutation.variables?.id}
                    categoryFavoritePending={categoryFavoriteMutation.isPending}
                    activeFavoriteCategoryId={categoryFavoriteMutation.variables?.id}
                    showOnlyChannelsWithEpg={guideState.showOnlyChannelsWithEpg}
                    onToggle={(nextOpen) =>
                      guideState.toggleCategory(category.id, nextOpen)}
                    onToggleCategoryFavorite={(nextCategory) =>
                      categoryFavoriteMutation.mutate(nextCategory)}
                    onFavorite={(channel) => favoriteMutation.mutate(channel)}
                    onPlay={(channelId) => playMutation.mutate(channelId)}
                  />
                </div>
              ))
            ) : (
              <Empty className="border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <SlidersHorizontal aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyTitle>
                    {guideState.shouldApplyFilter
                      ? "No categories match this filter"
                      : "No categories selected"}
                  </EmptyTitle>
                </EmptyHeader>
              </Empty>
            )}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
