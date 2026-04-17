import type { SportsEvent } from "@euripus/shared";
import { ExternalLink, Globe2, MapPin, ShieldCheck, Trophy } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import {
  formatAvailabilityLine,
  formatAvailabilityMeta,
  formatCompetitionLabel,
  formatEventSchedule,
  formatEventSecondaryText,
  formatEventStatusLabel,
  formatEventTimeRange,
  formatParticipants,
  formatSportLabel,
  getSportIcon,
  getStatusBadgeVariant,
} from "@/features/sports/sports-formatting";

export function SportsEventDetail({
  event,
  open,
  isPending,
  isError,
  onOpenChange,
}: {
  event: SportsEvent | null;
  open: boolean;
  isPending: boolean;
  isError: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const SportIcon = event ? getSportIcon(event.sport) : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[100dvh] w-[calc(100%-1rem)] max-w-4xl overflow-y-auto border-border/70 bg-background/98 p-0 sm:max-h-[88vh] sm:rounded-2xl">
        {isPending && !event ? (
          <div className="space-y-6 p-4 sm:p-6">
            <Skeleton className="h-6 w-32" />
            <Skeleton className="h-10 w-2/3" />
            <Skeleton className="h-24 w-full rounded-2xl" />
            <Skeleton className="h-40 w-full rounded-2xl" />
          </div>
        ) : null}

        {!isPending && isError && !event ? (
          <div className="p-4 sm:p-6">
            <DialogHeader>
              <DialogTitle>Couldn&apos;t load event details</DialogTitle>
              <DialogDescription>
                Sports watch guidance for this event is temporarily unavailable.
              </DialogDescription>
            </DialogHeader>
          </div>
        ) : null}

        {event ? (
          <div className="p-4 sm:p-6">
            <DialogHeader className="gap-4 text-left">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={getStatusBadgeVariant(event.status)}>
                  {formatEventStatusLabel(event.status)}
                </Badge>
                <Badge variant="outline">{formatCompetitionLabel(event.competition)}</Badge>
                <Badge variant="outline">{formatSportLabel(event.sport)}</Badge>
                {event.watch.recommendedProvider ? (
                  <Badge variant="accent">{event.watch.recommendedProvider}</Badge>
                ) : null}
              </div>
              <div className="space-y-2">
                <div className="flex items-start gap-3">
                  {SportIcon ? (
                    <div className="mt-0.5 flex size-10 shrink-0 items-center justify-center rounded-full bg-primary/8 text-primary">
                      <SportIcon className="size-5" aria-hidden="true" />
                    </div>
                  ) : null}
                  <DialogTitle className="text-2xl leading-tight text-pretty sm:text-3xl">
                    {formatParticipants(event)}
                  </DialogTitle>
                </div>
                <DialogDescription className="max-w-3xl text-sm leading-6">
                  {formatEventSecondaryText(event) ?? "Watch guidance and source details"}
                  {event.endTime ? null : " · End time not published yet"}
                </DialogDescription>
              </div>
            </DialogHeader>

            <div className="mt-6 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <DetailStat label="Start" value={formatEventSchedule(event)} icon={Trophy} />
              <DetailStat label="Window" value={formatEventTimeRange(event)} icon={ShieldCheck} />
              <DetailStat
                label="Venue"
                value={event.venue ?? event.roundLabel ?? "Venue TBD"}
                icon={MapPin}
              />
              <DetailStat
                label="Recommended market"
                value={event.watch.recommendedMarket?.toUpperCase() ?? "Multi-market"}
                icon={Globe2}
              />
            </div>

            <div className="mt-8 grid gap-6 xl:grid-cols-[1.35fr_0.9fr]">
              <section className="space-y-4 rounded-2xl border border-border/70 bg-card/70 p-5">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <h3 className="text-lg font-semibold tracking-tight">Watch guidance</h3>
                    <p className="text-sm text-muted-foreground">Best available watch options.</p>
                  </div>
                  <Badge variant="outline">{event.watch.availabilities.length} options</Badge>
                </div>

                <div className="space-y-3">
                  {event.watch.availabilities.length ? (
                    event.watch.availabilities.map((availability, index) => (
                      <div
                        key={`${availability.providerLabel}-${availability.channelName ?? index}`}
                        className="rounded-2xl border border-border/60 bg-background/60 p-4"
                      >
                        <div className="flex flex-wrap items-start justify-between gap-3">
                          <div>
                            <p className="font-semibold text-foreground">
                              {formatAvailabilityLine(availability)}
                            </p>
                            <p className="mt-1 text-sm text-muted-foreground">
                              {formatAvailabilityMeta(availability) || "Provider guidance"}
                            </p>
                          </div>
                          <div className="flex flex-wrap items-center gap-2">
                            {availability.providerFamily ? (
                              <Badge variant="outline">{availability.providerFamily}</Badge>
                            ) : null}
                            {availability.confidence !== null ? (
                              <Badge variant="outline">
                                {Math.round(availability.confidence * 100)}% confidence
                              </Badge>
                            ) : null}
                          </div>
                        </div>
                      </div>
                    ))
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      No watch options were attached to this event yet.
                    </p>
                  )}
                </div>
              </section>

              <section className="space-y-4 rounded-2xl border border-border/70 bg-card/70 p-5">
                <div>
                  <h3 className="text-lg font-semibold tracking-tight">Event metadata</h3>
                  <p className="text-sm text-muted-foreground">Source and search metadata.</p>
                </div>

                <div className="space-y-4 text-sm">
                  <MetadataRow label="Competition" value={formatCompetitionLabel(event.competition)} />
                  <MetadataRow label="Sport" value={formatSportLabel(event.sport)} />
                  <MetadataRow label="Status" value={formatEventStatusLabel(event.status)} />
                  <MetadataRow label="Venue" value={event.venue ?? "Unknown"} />
                  <MetadataRow label="Round" value={event.roundLabel ?? "Unknown"} />
                  <MetadataRow label="Source" value={event.source ?? "Unknown"} />
                  {event.sourceUrl ? (
                    <div className="space-y-2 rounded-xl border border-border/60 bg-background/60 p-3">
                      <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground/70">
                        Source URL
                      </p>
                      <a
                        href={event.sourceUrl}
                        target="_blank"
                        rel="noreferrer"
                        className="inline-flex items-center gap-2 break-all text-sm text-primary hover:underline"
                      >
                        <ExternalLink className="size-3.5" />
                        {event.sourceUrl}
                      </a>
                    </div>
                  ) : null}
                </div>

                {(event.searchMetadata.queries.length || event.searchMetadata.keywords.length) ? (
                  <>
                    <Separator />
                    <div className="space-y-4">
                      {event.searchMetadata.queries.length ? (
                        <MetadataList label="Fallback queries" values={event.searchMetadata.queries} />
                      ) : null}
                      {event.searchMetadata.keywords.length ? (
                        <MetadataList label="Keywords" values={event.searchMetadata.keywords} />
                      ) : null}
                    </div>
                  </>
                ) : null}
              </section>
            </div>
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

function DetailStat({
  label,
  value,
  icon: Icon,
}: {
  label: string;
  value: string;
  icon: typeof Trophy;
}) {
  return (
    <div className="rounded-2xl border border-border/70 bg-card/75 p-4">
      <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground/70">
        <Icon className="size-3.5" />
        <span>{label}</span>
      </div>
      <p className="mt-2 text-sm font-medium text-foreground">{value}</p>
    </div>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border/60 bg-background/60 p-3">
      <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground/70">
        {label}
      </p>
      <p className="mt-1 text-sm text-foreground/90">{value}</p>
    </div>
  );
}

function MetadataList({ label, values }: { label: string; values: string[] }) {
  return (
    <div className="space-y-2">
      <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground/70">
        {label}
      </p>
      <div className="flex flex-wrap gap-2">
        {values.map((value) => (
          <Badge key={value} variant="outline" className="max-w-full truncate">
            {value}
          </Badge>
        ))}
      </div>
    </div>
  );
}
