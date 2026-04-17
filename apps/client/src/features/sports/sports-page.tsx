import { useEffect, useMemo, useState } from "react";
import { CalendarClock, Radio, RefreshCcw, Sparkles, TimerReset, Trophy } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { SportsEventDetail } from "@/features/sports/sports-event-detail";
import { SportsSection } from "@/features/sports/sports-section";
import { useSportsPageState } from "@/features/sports/use-sports-page-state";

type SportsView = "live" | "next" | "later";

const VIEW_CONFIG: Record<
  SportsView,
  {
    label: string;
    title: string;
  }
> = {
  live: {
    label: "Live",
    title: "Live Now",
  },
  next: {
    label: "Next 24h",
    title: "Next 24 Hours",
  },
  later: {
    label: "Later",
    title: "Plan Ahead",
  },
};

export function SportsPage() {
  const sportsState = useSportsPageState();
  const [activeView, setActiveView] = useState<SportsView>("live");

  const headerMeta = (
    <>
      <Badge variant="live">{sportsState.filteredLiveEvents.length} live</Badge>
      {sportsState.providerCount ? (
        <Badge variant="outline">{sportsState.providerCount} providers</Badge>
      ) : null}
      {sportsState.selectedCompetition !== "all" ? (
        <Badge variant="accent">{sportsState.selectedCompetitionLabel}</Badge>
      ) : null}
    </>
  );

  const overviewUnavailable =
    sportsState.liveQuery.isError &&
    sportsState.todayQuery.isError &&
    sportsState.upcomingQuery.isError &&
    !sportsState.hasAnyData;

  const sportsNotConfigured =
    sportsState.liveQuery.error instanceof Error &&
    sportsState.liveQuery.error.message.includes("not configured") &&
    sportsState.todayQuery.isError &&
    sportsState.upcomingQuery.isError;

  const viewCounts = useMemo(
    () => ({
      live: sportsState.filteredLiveEvents.length,
      next: sportsState.filteredTodayEvents.length,
      later: sportsState.filteredLaterEvents.length,
    }),
    [
      sportsState.filteredLaterEvents.length,
      sportsState.filteredLiveEvents.length,
      sportsState.filteredTodayEvents.length,
    ],
  );

  useEffect(() => {
    if (viewCounts[activeView] > 0) {
      return;
    }

    if (viewCounts.live > 0) {
      setActiveView("live");
      return;
    }

    if (viewCounts.next > 0) {
      setActiveView("next");
      return;
    }

    if (viewCounts.later > 0) {
      setActiveView("later");
    }
  }, [activeView, viewCounts]);

  const activeViewConfig = VIEW_CONFIG[activeView];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Sports"
        actions={(
          <Button variant="outline" onClick={sportsState.refreshSports}>
            <RefreshCcw data-icon="inline-start" />
            Refresh
          </Button>
        )}
        meta={headerMeta}
      />

      {overviewUnavailable ? (
        <Card className="border-dashed border-border/70 bg-card/50">
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Trophy aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>
                  {sportsNotConfigured
                    ? "Sports Is Not Configured"
                    : "Sports Data Is Temporarily Unavailable"}
                </EmptyTitle>
                <EmptyDescription>
                  {sportsNotConfigured
                    ? "Set APP_SPORTS_API_BASE_URL on the Euripus server to enable the sports experience."
                    : "The Sports API could not be reached just now. Try refreshing again shortly."}
                </EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : (
        <Tabs
          value={activeView}
          onValueChange={(value) => setActiveView(value as SportsView)}
          className="flex flex-col gap-4"
        >
          <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/70 sm:bg-[radial-gradient(circle_at_top_left,rgba(168,85,247,0.12),transparent_32%),linear-gradient(180deg,rgba(255,255,255,0.02),rgba(255,255,255,0))] sm:shadow-sm">
            <CardHeader className="gap-4 px-0 py-0 sm:p-6">
              <div className="grid grid-cols-3 gap-2 sm:gap-3">
                <OverviewStat
                  icon={Radio}
                  label="Live"
                  value={viewCounts.live}
                  tone="live"
                />
                <OverviewStat
                  icon={CalendarClock}
                  label="Next 24h"
                  value={viewCounts.next}
                />
                <OverviewStat
                  icon={TimerReset}
                  label="Later"
                  value={viewCounts.later}
                />
              </div>

              <div className="flex flex-col gap-2.5 py-1 sm:rounded-2xl sm:border sm:border-border/60 sm:bg-background/60 sm:p-4">
                <div className="flex flex-col gap-1 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between sm:gap-2">
                  <div className="flex items-center gap-2 text-sm font-medium text-foreground">
                    <Sparkles className="size-4 text-primary" aria-hidden="true" />
                    Competition
                  </div>
                  <p className="text-xs text-muted-foreground sm:text-sm">
                    {sportsState.selectedCompetition === "all"
                      ? `${sportsState.totalFilteredCount} unique events in view`
                      : `Showing ${sportsState.selectedCompetitionLabel}`}
                  </p>
                </div>

                <ScrollArea className="-mx-1 whitespace-nowrap px-1 sm:mx-0 sm:px-0">
                  <ToggleGroup
                    type="single"
                    value={sportsState.selectedCompetition}
                    onValueChange={(value) => {
                      if (value) {
                        sportsState.setSelectedCompetition(value);
                      }
                    }}
                    variant="outline"
                    className="flex w-max min-w-full gap-2 pb-1 sm:pb-2"
                  >
                    {sportsState.competitionOptions.map((option) => (
                      <ToggleGroupItem
                        key={option.value}
                        value={option.value}
                        className="h-9 shrink-0 whitespace-nowrap rounded-full px-3 text-sm data-[state=on]:border-primary/30 data-[state=on]:bg-primary/10 data-[state=on]:text-primary sm:px-4"
                      >
                        <span className="truncate">{option.label}</span>
                        <span className="ml-1.5 text-[11px] opacity-70 sm:ml-2 sm:text-xs">{option.count}</span>
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                </ScrollArea>
              </div>

              <div className="flex flex-col gap-3">
                <TabsList className="grid h-auto w-full grid-cols-3 gap-2 rounded-2xl bg-muted/35 p-1 sm:bg-muted/50">
                  {(Object.entries(VIEW_CONFIG) as [SportsView, (typeof VIEW_CONFIG)[SportsView]][]).map(
                    ([value, config]) => (
                      <TabsTrigger
                        key={value}
                        value={value}
                        className="flex min-h-[74px] h-auto flex-col items-start justify-between gap-2 rounded-xl px-3 py-3 text-left"
                      >
                        <span className="min-h-[1.75rem] text-[11px] font-semibold uppercase leading-4 tracking-[0.16em] text-muted-foreground sm:min-h-0 sm:text-xs">
                          {config.label}
                        </span>
                        <span className="text-sm font-semibold text-foreground">
                          {viewCounts[value]} events
                        </span>
                      </TabsTrigger>
                    ),
                  )}
                </TabsList>

                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-3">
                  <p className="text-sm font-medium text-foreground">{activeViewConfig.title}</p>
                  <Badge variant={activeView === "live" ? "live" : "outline"}>
                    {viewCounts[activeView]} showing
                  </Badge>
                </div>
              </div>
            </CardHeader>

            <CardContent className="px-0 pt-0 pb-0 sm:px-6 sm:pb-6">
              <TabsContent value="live" className="mt-0">
                <SportsSection
                  title="Live Now"
                  description="Underway right now, with the clearest watch guidance first."
                  icon="live"
                  events={sportsState.filteredLiveEvents}
                  isPending={sportsState.liveQuery.isPending}
                  isError={sportsState.liveQuery.isError}
                  emptyTitle={
                    sportsState.selectedCompetition === "all"
                      ? "No Live Events Right Now"
                      : `No Live ${sportsState.selectedCompetitionLabel} Events`
                  }
                  emptyDescription="Check the next tab to see what starts soon."
                  onViewDetails={sportsState.openEventDetail}
                  showHeader={false}
                />
              </TabsContent>

              <TabsContent value="next" className="mt-0">
                <SportsSection
                  title="Next 24 Hours"
                  description="The next watchable window, without burying you in later fixtures."
                  icon="today"
                  events={sportsState.filteredTodayEvents}
                  isPending={sportsState.todayQuery.isPending}
                  isError={sportsState.todayQuery.isError}
                  emptyTitle={
                    sportsState.selectedCompetition === "all"
                      ? "Nothing in the Next 24 Hours"
                      : `No ${sportsState.selectedCompetitionLabel} Events Soon`
                  }
                  emptyDescription="Try another competition or check the later planning view."
                  onViewDetails={sportsState.openEventDetail}
                  showHeader={false}
                />
              </TabsContent>

              <TabsContent value="later" className="mt-0">
                <SportsSection
                  title="Plan Ahead"
                  description="Later fixtures only — no repeats from the near-term view."
                  icon="upcoming"
                  events={sportsState.filteredLaterEvents}
                  isPending={sportsState.upcomingQuery.isPending}
                  isError={sportsState.upcomingQuery.isError}
                  emptyTitle={
                    sportsState.selectedCompetition === "all"
                      ? "No Later Events Found"
                      : `No Later ${sportsState.selectedCompetitionLabel} Events`
                  }
                  emptyDescription="The Sports API hasn’t attached any later fixtures in this window yet."
                  onViewDetails={sportsState.openEventDetail}
                  showHeader={false}
                />
              </TabsContent>
            </CardContent>
          </Card>
        </Tabs>
      )}

      <SportsEventDetail
        open={!!sportsState.selectedEventId}
        event={sportsState.eventDetailQuery.data ?? sportsState.selectedEventPreview}
        isPending={sportsState.eventDetailQuery.isPending}
        isError={sportsState.eventDetailQuery.isError}
        onOpenChange={(open) => {
          if (!open) {
            sportsState.closeEventDetail();
          }
        }}
      />
    </div>
  );
}

function OverviewStat({
  icon: Icon,
  label,
  value,
  tone = "default",
}: {
  icon: typeof Radio;
  label: string;
  value: number;
  tone?: "default" | "live";
}) {
  return (
    <div className="flex min-h-[92px] flex-col justify-between rounded-2xl border border-border/60 bg-background/70 px-3 py-3 shadow-sm sm:min-h-0 sm:px-4">
      <div className="flex min-h-8 items-start gap-1.5 text-[10px] font-semibold uppercase leading-4 tracking-[0.14em] text-muted-foreground sm:min-h-0 sm:gap-2 sm:text-xs">
        <Icon
          className={tone === "live" ? "mt-0.5 size-3.5 shrink-0 text-live" : "mt-0.5 size-3.5 shrink-0 text-primary"}
          aria-hidden="true"
        />
        <span className="text-balance">{label}</span>
      </div>
      <p className="mt-2 text-2xl font-semibold tracking-tight text-foreground">{value}</p>
    </div>
  );
}
