import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import type { SportsEvent } from "@euripus/shared";
import {
  getSportsEvent,
  getSportsLiveEvents,
  getSportsProviders,
  getSportsTodayEvents,
  getSportsUpcomingEvents,
} from "@/lib/api";
import {
  SPORTS_DETAIL_STALE_TIME_MS,
  SPORTS_LIVE_STALE_TIME_MS,
  SPORTS_PROVIDER_STALE_TIME_MS,
  SPORTS_TODAY_STALE_TIME_MS,
  SPORTS_UPCOMING_STALE_TIME_MS,
} from "@/lib/query-cache";
import { formatCompetitionLabel } from "@/features/sports/sports-formatting";

const DEFAULT_UPCOMING_HOURS = 72;
const ALL_COMPETITIONS = "all";

export function useSportsPageState() {
  const queryClient = useQueryClient();
  const [selectedCompetition, setSelectedCompetition] =
    useState<string>(ALL_COMPETITIONS);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);

  const liveQuery = useQuery({
    queryKey: ["sports", "live"],
    queryFn: getSportsLiveEvents,
    staleTime: SPORTS_LIVE_STALE_TIME_MS,
  });
  const todayQuery = useQuery({
    queryKey: ["sports", "today"],
    queryFn: getSportsTodayEvents,
    staleTime: SPORTS_TODAY_STALE_TIME_MS,
  });
  const upcomingQuery = useQuery({
    queryKey: ["sports", "upcoming", DEFAULT_UPCOMING_HOURS],
    queryFn: () => getSportsUpcomingEvents(DEFAULT_UPCOMING_HOURS),
    staleTime: SPORTS_UPCOMING_STALE_TIME_MS,
  });
  const providersQuery = useQuery({
    queryKey: ["sports", "providers"],
    queryFn: getSportsProviders,
    staleTime: SPORTS_PROVIDER_STALE_TIME_MS,
  });

  const liveEvents = liveQuery.data?.events ?? [];
  const todayEvents = todayQuery.data?.events ?? [];
  const upcomingEvents = upcomingQuery.data?.events ?? [];

  const laterEvents = useMemo(() => {
    const hiddenIds = new Set([...liveEvents, ...todayEvents].map((event) => event.id));
    return upcomingEvents.filter((event) => !hiddenIds.has(event.id));
  }, [liveEvents, todayEvents, upcomingEvents]);

  const allEvents = useMemo(() => {
    const deduped = new Map<string, SportsEvent>();
    for (const event of [...liveEvents, ...todayEvents, ...upcomingEvents]) {
      if (!deduped.has(event.id)) {
        deduped.set(event.id, event);
      }
    }

    return [...deduped.values()];
  }, [liveEvents, todayEvents, upcomingEvents]);

  const competitionOptions = useMemo(() => {
    const options = new Map<string, { value: string; label: string; count: number }>();
    for (const event of allEvents) {
      const entry = options.get(event.competition);
      if (entry) {
        entry.count += 1;
      } else {
        options.set(event.competition, {
          value: event.competition,
          label: formatCompetitionLabel(event.competition),
          count: 1,
        });
      }
    }

    return [
      { value: ALL_COMPETITIONS, label: "All", count: allEvents.length },
      ...[...options.values()].sort((left, right) =>
        left.label.localeCompare(right.label),
      ),
    ];
  }, [allEvents]);

  useEffect(() => {
    if (
      selectedCompetition !== ALL_COMPETITIONS &&
      !competitionOptions.some((option) => option.value === selectedCompetition)
    ) {
      setSelectedCompetition(ALL_COMPETITIONS);
    }
  }, [competitionOptions, selectedCompetition]);

  const selectedCompetitionLabel = useMemo(
    () =>
      competitionOptions.find((option) => option.value === selectedCompetition)?.label ?? "All",
    [competitionOptions, selectedCompetition],
  );

  const selectedEventPreview = useMemo(
    () => allEvents.find((event) => event.id === selectedEventId) ?? null,
    [allEvents, selectedEventId],
  );

  const eventDetailQuery = useQuery({
    queryKey: ["sports", "event", selectedEventId],
    queryFn: () => getSportsEvent(selectedEventId ?? ""),
    enabled: !!selectedEventId,
    staleTime: SPORTS_DETAIL_STALE_TIME_MS,
    placeholderData: selectedEventPreview ?? undefined,
  });

  const filteredLiveEvents = useMemo(
    () => filterEvents(liveEvents, selectedCompetition),
    [liveEvents, selectedCompetition],
  );
  const filteredTodayEvents = useMemo(
    () => filterEvents(todayEvents, selectedCompetition),
    [todayEvents, selectedCompetition],
  );
  const filteredLaterEvents = useMemo(
    () => filterEvents(laterEvents, selectedCompetition),
    [laterEvents, selectedCompetition],
  );
  const filteredUpcomingEvents = useMemo(
    () => filterEvents(upcomingEvents, selectedCompetition),
    [upcomingEvents, selectedCompetition],
  );

  const totalFilteredCount = useMemo(() => {
    const deduped = new Set<string>();
    for (const event of [
      ...filteredLiveEvents,
      ...filteredTodayEvents,
      ...filteredLaterEvents,
    ]) {
      deduped.add(event.id);
    }

    return deduped.size;
  }, [filteredLaterEvents, filteredLiveEvents, filteredTodayEvents]);

  async function refreshSports() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["sports", "live"] }),
      queryClient.invalidateQueries({ queryKey: ["sports", "today"] }),
      queryClient.invalidateQueries({ queryKey: ["sports", "upcoming"] }),
      queryClient.invalidateQueries({ queryKey: ["sports", "providers"] }),
      selectedEventId
        ? queryClient.invalidateQueries({ queryKey: ["sports", "event", selectedEventId] })
        : Promise.resolve(),
    ]);
  }

  function openEventDetail(eventId: string) {
    setSelectedEventId(eventId);
  }

  function closeEventDetail() {
    setSelectedEventId(null);
  }

  return {
    selectedCompetition,
    selectedCompetitionLabel,
    setSelectedCompetition,
    competitionOptions,
    liveQuery,
    todayQuery,
    upcomingQuery,
    providersQuery,
    filteredLiveEvents,
    filteredTodayEvents,
    filteredLaterEvents,
    filteredUpcomingEvents,
    selectedEventId,
    selectedEventPreview,
    eventDetailQuery,
    openEventDetail,
    closeEventDetail,
    refreshSports,
    totalFilteredCount,
    hasAnyData: allEvents.length > 0,
    hasAnyError:
      liveQuery.isError ||
      todayQuery.isError ||
      upcomingQuery.isError ||
      providersQuery.isError,
    providerCount: providersQuery.data?.count ?? 0,
  };
}

function filterEvents(events: SportsEvent[], selectedCompetition: string) {
  if (selectedCompetition === ALL_COMPETITIONS) {
    return events;
  }

  return events.filter((event) => event.competition === selectedCompetition);
}
