import type { SportsEvent } from "@euripus/shared";
import { Clock3, Globe2, MapPin, Radio, Tv } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import {
  formatAvailabilityLine,
  formatCompetitionLabel,
  formatEventRelativeStart,
  formatEventSchedule,
  formatEventSecondaryText,
  formatEventStatusLabel,
  formatEventTimeRange,
  formatParticipants,
  getPrimaryAvailability,
  getSportIcon,
  getStatusBadgeVariant,
} from "@/features/sports/sports-formatting";
import { cn, getTimeProgress } from "@/lib/utils";

const HEAVY_CARD_STYLE = {
  contentVisibility: "auto",
  containIntrinsicSize: "260px",
} as const;

export function SportsEventCard({
  event,
  onViewDetails,
}: {
  event: SportsEvent;
  onViewDetails: (eventId: string) => void;
}) {
  const primaryAvailability = getPrimaryAvailability(event);
  const isLive = event.status === "live";
  const SportIcon = getSportIcon(event.sport);
  const secondaryText = formatEventSecondaryText(event);
  const liveProgress = isLive && event.endTime
    ? getTimeProgress(event.startTime, event.endTime)
    : null;

  return (
    <Card
      style={HEAVY_CARD_STYLE}
      className={cn(
        "group overflow-hidden rounded-none border-x-0 border-t-0 border-border/60 bg-transparent shadow-none transition-[transform,box-shadow,border-color] duration-200 hover:border-primary/20 sm:rounded-3xl sm:border sm:border-border/70 sm:bg-card/95 sm:shadow-sm sm:hover:-translate-y-0.5 sm:hover:border-primary/30 sm:hover:shadow-[0_18px_48px_rgba(0,0,0,0.12)]",
        isLive &&
          "border-live/20 sm:bg-[radial-gradient(circle_at_top_left,rgba(239,68,68,0.10),transparent_36%),linear-gradient(180deg,rgba(255,255,255,0.02),rgba(255,255,255,0))]",
      )}
    >
      <CardHeader className="gap-3 px-0 pt-0 pb-4 sm:gap-4 sm:p-6">
        <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={getStatusBadgeVariant(event.status)}>
                {isLive ? <Radio className="size-3" aria-hidden="true" /> : null}
                {formatEventStatusLabel(event.status)}
              </Badge>
              <Badge variant="outline">{formatCompetitionLabel(event.competition)}</Badge>
              {event.watch.recommendedProvider ? (
                <Badge variant="accent">{event.watch.recommendedProvider}</Badge>
              ) : null}
            </div>

            <div className="mt-3 flex flex-col gap-1.5">
              <div className="flex items-start gap-3">
                <div className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-full bg-primary/8 text-primary sm:size-10">
                  <SportIcon className="size-4 sm:size-5" aria-hidden="true" />
                </div>
                <CardTitle className="pt-0.5 text-xl leading-tight text-pretty sm:text-[clamp(1.35rem,2vw,1.7rem)]">
                  {formatParticipants(event)}
                </CardTitle>
              </div>
              {secondaryText ? (
                <CardDescription className="line-clamp-2 text-sm leading-6">
                  {secondaryText}
                </CardDescription>
              ) : null}
            </div>
          </div>

          <div className="grid gap-1.5 border-t border-border/50 pt-3 text-left md:min-h-[132px] md:min-w-[220px] md:self-stretch md:grid-rows-[auto_auto_1fr] md:rounded-2xl md:border md:border-border/60 md:bg-background/80 md:p-4 md:pt-4 md:text-right">
            <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
              Start
            </p>
            <p className="text-base font-semibold leading-tight text-foreground text-pretty md:text-[1.05rem]">
              {formatEventSchedule(event)}
            </p>
            <p className="text-sm text-muted-foreground md:self-end">
              {formatEventRelativeStart(event)}
            </p>
          </div>
        </div>
      </CardHeader>

      <CardContent className="flex flex-col gap-3 px-0 pt-0 pb-4 sm:px-6 sm:pb-6">
        <div className="p-0 sm:rounded-2xl sm:border sm:border-border/60 sm:bg-muted/35 sm:p-4">
          <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
            <Tv className="size-3.5" aria-hidden="true" />
            Best Watch Option
          </div>
          <p className="mt-2 text-base font-semibold text-foreground text-pretty">
            {primaryAvailability
              ? formatAvailabilityLine(primaryAvailability)
              : "Watch guidance pending"}
          </p>
          <p className="mt-1 line-clamp-2 text-sm leading-5 text-muted-foreground sm:leading-6">
            {primaryAvailability?.watchType
              ? `${primaryAvailability.watchType} · ${primaryAvailability.market?.toUpperCase() ?? "Multi-market"}`
              : "The Sports API has not attached a recommended watch option yet."}
          </p>
        </div>

        {liveProgress !== null ? (
          <div className="flex flex-col gap-2 border-t border-border/50 pt-3 sm:rounded-2xl sm:border sm:border-live/15 sm:bg-live/5 sm:p-4">
            <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
              <span className="font-medium text-foreground">Live window</span>
              <span className="text-muted-foreground">{formatEventTimeRange(event)}</span>
            </div>
            <Progress value={liveProgress} className="h-2 bg-live/10 [&>div]:bg-live" />
          </div>
        ) : null}

        <div className="grid grid-cols-3 gap-2 border-t border-border/50 pt-3 sm:gap-3 sm:border-0 sm:pt-0 md:grid-cols-3">
          <InfoRow icon={Clock3} label="Window" value={formatEventTimeRange(event)} />
          <InfoRow icon={MapPin} label="Venue" value={event.venue ?? event.roundLabel ?? "Venue TBD"} />
          <InfoRow
            icon={Globe2}
            label="Market"
            value={event.watch.recommendedMarket?.toUpperCase() ?? "Multi-market"}
          />
        </div>
      </CardContent>

      <CardFooter className="flex-col items-stretch justify-between gap-3 border-t border-border/50 px-0 pt-4 pb-1 sm:border-border/60 sm:px-6 sm:py-4 sm:flex-row sm:items-center">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          {event.roundLabel ? (
            <Badge variant="outline" className="max-w-full truncate">
              {event.roundLabel}
            </Badge>
          ) : null}
          {primaryAvailability?.channelName ? (
            <Badge variant="outline" className="max-w-full truncate">
              {primaryAvailability.channelName}
            </Badge>
          ) : null}
        </div>

        <Button
          variant={isLive ? "default" : "outline"}
          onClick={() => onViewDetails(event.id)}
          className="w-full sm:w-auto"
        >
          View Details
        </Button>
      </CardFooter>
    </Card>
  );
}

function InfoRow({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Clock3;
  label: string;
  value: string;
}) {
  return (
    <div className="min-w-0 rounded-xl bg-background/35 p-2.5 sm:rounded-2xl sm:border sm:border-border/60 sm:bg-background/60 sm:p-4">
      <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground sm:gap-2 sm:text-xs">
        <Icon className="size-3.5 shrink-0" aria-hidden="true" />
        <span className="truncate">{label}</span>
      </div>
      <p className="mt-1.5 break-words text-sm font-medium leading-5 text-foreground/90 text-pretty sm:mt-2 sm:leading-6">
        {value}
      </p>
    </div>
  );
}
