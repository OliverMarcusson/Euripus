import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarDays, ExternalLink } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  connectGoogleCalendar,
  disconnectGoogleCalendar,
  getGoogleCalendars,
  getGoogleCalendarStatus,
  selectGoogleCalendar,
} from "@/lib/api";

export function GoogleCalendarSettings() {
  const queryClient = useQueryClient();
  const statusQuery = useQuery({
    queryKey: ["google-calendar", "status"],
    queryFn: getGoogleCalendarStatus,
  });
  const status = statusQuery.data;
  const calendarsQuery = useQuery({
    queryKey: ["google-calendar", "calendars"],
    queryFn: getGoogleCalendars,
    enabled:
      !!status?.configured &&
      !!status.connected &&
      !status.needsReauthorization,
  });
  const connectMutation = useMutation({
    mutationFn: connectGoogleCalendar,
    onSuccess: ({ authorizationUrl }) => {
      window.location.assign(authorizationUrl);
    },
  });
  const selectionMutation = useMutation({
    mutationFn: (calendarId: string) => selectGoogleCalendar({ calendarId }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["google-calendar"] });
    },
  });
  const disconnectMutation = useMutation({
    mutationFn: disconnectGoogleCalendar,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["google-calendar"] });
    },
  });

  useEffect(() => {
    if (
      !status?.connected ||
      status.selectedCalendarId ||
      selectionMutation.isPending
    )
      return;
    const primary = calendarsQuery.data?.find((calendar) => calendar.primary);
    if (primary) selectionMutation.mutate(primary.id);
  }, [
    calendarsQuery.data,
    selectionMutation,
    status?.connected,
    status?.selectedCalendarId,
  ]);

  return (
    <Card className="rounded-none border-0 border-t border-border/60 bg-transparent py-8 shadow-none sm:py-10">
      <CardHeader className="flex flex-row items-center justify-between gap-4 px-0 pb-6 pt-0">
        <CardTitle className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
          <CalendarDays className="size-5" /> Google Calendar
        </CardTitle>
        <Badge
          variant={
            status?.connected && !status.needsReauthorization
              ? "accent"
              : "outline"
          }
        >
          {!status?.configured
            ? "Not configured"
            : status?.needsReauthorization
              ? "Reconnect required"
              : status?.connected
                ? "Connected"
                : "Disconnected"}
        </Badge>
      </CardHeader>
      <CardContent className="flex max-w-3xl flex-col gap-4 px-0 pb-0">
        {statusQuery.isPending ? (
          <p className="text-sm text-muted-foreground">
            Checking Google Calendar…
          </p>
        ) : null}
        {statusQuery.isError ? (
          <p className="text-sm text-destructive">
            Unable to check Google Calendar.
          </p>
        ) : null}
        {status?.configured &&
        (!status.connected || status.needsReauthorization) ? (
          <Button
            className="w-fit"
            onClick={() => connectMutation.mutate()}
            disabled={connectMutation.isPending}
          >
            <ExternalLink data-icon="inline-start" />{" "}
            {status.needsReauthorization
              ? "Reconnect Google Calendar"
              : "Connect Google Calendar"}
          </Button>
        ) : null}
        {status?.configured &&
        status.connected &&
        !status.needsReauthorization ? (
          <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
            <div className="min-w-0 flex-1">
              <label
                className="mb-2 block text-sm font-medium"
                htmlFor="google-calendar-select"
              >
                Calendar for sports events
              </label>
              <Select
                value={status.selectedCalendarId ?? undefined}
                onValueChange={(value) => selectionMutation.mutate(value)}
                disabled={
                  calendarsQuery.isPending || selectionMutation.isPending
                }
              >
                <SelectTrigger id="google-calendar-select">
                  <SelectValue placeholder="Choose a writable calendar" />
                </SelectTrigger>
                <SelectContent>
                  {(calendarsQuery.data ?? []).map((calendar) => (
                    <SelectItem key={calendar.id} value={calendar.id}>
                      {calendar.summary}
                      {calendar.primary ? " (Primary)" : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              variant="outline"
              onClick={() => disconnectMutation.mutate()}
              disabled={disconnectMutation.isPending}
            >
              Disconnect
            </Button>
          </div>
        ) : null}
        {connectMutation.isError ||
        calendarsQuery.isError ||
        selectionMutation.isError ||
        disconnectMutation.isError ? (
          <p className="text-sm text-destructive">
            The Google Calendar operation failed. Try reconnecting the account.
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
