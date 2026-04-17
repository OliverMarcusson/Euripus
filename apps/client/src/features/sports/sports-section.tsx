import type { SportsEvent } from "@euripus/shared";
import { AlertTriangle, CalendarClock, Radio, TimerReset } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { SportsEventCard } from "@/features/sports/sports-event-card";

export function SportsSection({
  title,
  description,
  icon,
  events,
  isPending,
  isError,
  emptyTitle,
  emptyDescription,
  onViewDetails,
  showHeader = true,
}: {
  title: string;
  description: string;
  icon: "live" | "today" | "upcoming";
  events: SportsEvent[];
  isPending: boolean;
  isError: boolean;
  emptyTitle: string;
  emptyDescription: string;
  onViewDetails: (eventId: string) => void;
  showHeader?: boolean;
}) {
  const Icon = icon === "live" ? Radio : icon === "today" ? CalendarClock : TimerReset;

  return (
    <section className="flex flex-col gap-4">
      {showHeader ? (
        <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border/60 pb-4">
          <div className="flex min-w-0 items-start gap-3">
            <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl border border-border/60 bg-background text-primary">
              <Icon className="size-4" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <h2 className="text-xl font-semibold tracking-tight text-balance">{title}</h2>
              <p className="mt-1 max-w-2xl text-sm leading-6 text-muted-foreground">{description}</p>
            </div>
          </div>
          <Badge variant={icon === "live" ? "live" : "outline"}>{events.length}</Badge>
        </div>
      ) : null}

      {isPending ? (
        <div className="grid gap-3 sm:gap-4 2xl:grid-cols-2">
          {Array.from({ length: 3 }).map((_, index) => (
            <Card key={index} className="border-border/60 bg-card/80">
              <CardHeader className="gap-4">
                <div className="flex items-center gap-2">
                  <Skeleton className="h-5 w-16" />
                  <Skeleton className="h-5 w-24" />
                </div>
                <div className="flex flex-col gap-2">
                  <Skeleton className="h-8 w-3/5" />
                  <Skeleton className="h-5 w-1/2" />
                </div>
                <Skeleton className="h-24 rounded-2xl" />
              </CardHeader>
              <CardContent className="flex flex-col gap-3 pt-0">
                <Skeleton className="h-16 rounded-2xl" />
                <Skeleton className="h-10 w-28" />
              </CardContent>
            </Card>
          ))}
        </div>
      ) : null}

      {!isPending && isError ? (
        <Card className="border-destructive/20 bg-destructive/5">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <AlertTriangle className="size-4 text-destructive" aria-hidden="true" />
              Couldn&apos;t Load This View
            </CardTitle>
            <CardDescription>
              Sports data for this part of the page is temporarily unavailable.
            </CardDescription>
          </CardHeader>
        </Card>
      ) : null}

      {!isPending && !isError && !events.length ? (
        <Card className="border-dashed border-border/70 bg-card/50">
          <CardContent className="p-0">
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Icon aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>{emptyTitle}</EmptyTitle>
                <EmptyDescription>{emptyDescription}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>
      ) : null}

      {!isPending && !isError && events.length ? (
        <div className="grid gap-8 sm:gap-4 2xl:grid-cols-2">
          {events.map((event) => (
            <SportsEventCard key={event.id} event={event} onViewDetails={onViewDetails} />
          ))}
        </div>
      ) : null}
    </section>
  );
}
