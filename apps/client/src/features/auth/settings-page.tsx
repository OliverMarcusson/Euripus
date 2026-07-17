import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LaptopMinimal, Moon, Play, Radio, Sun } from "lucide-react";
import { useState } from "react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderSettingsSection } from "@/features/provider/provider-settings-section";
import { RestrictedProviderSyncSection } from "@/features/provider/restricted-provider-sync-section";
import { GoogleCalendarSettings } from "@/features/calendar/google-calendar-settings";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";
import { usePpvFavoriteMutation } from "@/hooks/use-ppv-favorite";
import {
  getRecents,
  getRemoteReceivers,
  pairReceiver,
  unpairReceiver,
} from "@/lib/api";
import {
  REMOTE_QUERY_STALE_TIME_MS,
  STANDARD_QUERY_STALE_TIME_MS,
} from "@/lib/query-cache";
import { formatReceiverPlaybackSummary } from "@/lib/receiver-playback";
import {
  formatDateTime,
  formatEventChannelTitle,
  formatRelativeTime,
} from "@/lib/utils";
import type { ThemePreference } from "@/store/theme-store";
import { useThemeStore } from "@/store/theme-store";
import { useAuthStore } from "@/store/auth-store";
import { usePlaybackSettingsStore } from "@/store/playback-settings-store";
import { useChannelSettingsStore } from "@/store/channel-settings-store";

const themeOptions: Array<{
  value: ThemePreference;
  label: string;
  icon: typeof LaptopMinimal;
}> = [
  { value: "system", label: "System", icon: LaptopMinimal },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
] as const;

function SettingsSwitch({
  id,
  checked,
  onToggle,
}: {
  id: string;
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={onToggle}
      className="relative inline-flex h-7 w-12 shrink-0 items-center rounded-full border border-border bg-muted transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=checked]:border-primary data-[state=checked]:bg-primary"
      data-state={checked ? "checked" : "unchecked"}
    >
      <span
        className="mx-1 block size-5 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-5"
        data-state={checked ? "checked" : "unchecked"}
      />
    </button>
  );
}

function SettingSwitchRow({
  title = "PPV event dates",
  label,
  id,
  checked,
  onToggle,
}: {
  title?: string;
  label: string;
  id: string;
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="grid gap-4 py-6 md:grid-cols-[minmax(0,240px)_minmax(0,520px)] md:items-center md:justify-between">
      <h2 className="text-sm font-medium">{title}</h2>
      <div className="flex items-center justify-between gap-4">
        <label htmlFor={id} className="text-sm text-muted-foreground">
          {label}
        </label>
        <SettingsSwitch id={id} checked={checked} onToggle={onToggle} />
      </div>
    </div>
  );
}

export function SettingsPage() {
  const user = useAuthStore((state) => state.user);
  const queryClient = useQueryClient();
  const [pairCode, setPairCode] = useState("");
  const [rememberDevice, setRememberDevice] = useState(true);
  const [pairName, setPairName] = useState("");
  const recentsQuery = useQuery({
    queryKey: ["recents"],
    queryFn: getRecents,
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
  });
  const remoteDevicesQuery = useQuery({
    queryKey: ["remote", "receivers"],
    queryFn: getRemoteReceivers,
    staleTime: REMOTE_QUERY_STALE_TIME_MS,
  });
  const favoriteMutation = useChannelFavoriteMutation();
  const ppvFavoriteMutation = usePpvFavoriteMutation();
  const pairMutation = useMutation({
    mutationFn: pairReceiver,
    onSuccess: async () => {
      setPairCode("");
      setPairName("");
      await queryClient.invalidateQueries({ queryKey: ["remote"] });
    },
  });
  const unpairMutation = useMutation({
    mutationFn: unpairReceiver,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["remote"] });
    },
  });
  const preference = useThemeStore((state) => state.preference);
  const setPreference = useThemeStore((state) => state.setPreference);
  const livePlaybackPreference = usePlaybackSettingsStore(
    (state) => state.livePlaybackPreference,
  );
  const setLivePlaybackPreference = usePlaybackSettingsStore(
    (state) => state.setLivePlaybackPreference,
  );
  const filterPpvByDate = useChannelSettingsStore(
    (state) => state.filterPpvByDate,
  );
  const setFilterPpvByDate = useChannelSettingsStore(
    (state) => state.setFilterPpvByDate,
  );
  const qualityChannelsOnly = useChannelSettingsStore(
    (state) => state.qualityChannelsOnly,
  );
  const setQualityChannelsOnly = useChannelSettingsStore(
    (state) => state.setQualityChannelsOnly,
  );
  const adminToolsEnabled = useChannelSettingsStore(
    (state) => state.adminToolsEnabled,
  );
  const setAdminToolsEnabled = useChannelSettingsStore(
    (state) => state.setAdminToolsEnabled,
  );
  const recents = recentsQuery.data ?? [];
  const remoteDevices = remoteDevicesQuery.data ?? [];
  const playMutation = useChannelPlaybackMutation();

  return (
    <div className="flex flex-col">
      <PageHeader title="Settings" />

      <div className="flex flex-col">
        <Card className="flex flex-col overflow-hidden rounded-none border-0 bg-transparent py-8 shadow-none sm:py-10">
          <CardHeader className="flex shrink-0 flex-row items-center justify-between gap-4 px-0 pb-5 pt-0">
            <CardTitle className="text-2xl font-semibold tracking-tight">
              Recent channels
            </CardTitle>
            <Badge
              variant="outline"
              className="bg-background/60 backdrop-blur-md"
            >
              {recents.length}
            </Badge>
          </CardHeader>
          <CardContent className="flex flex-col p-0 pb-2">
            {recents.length ? (
              <ScrollArea
                className="max-h-[32rem] pr-3"
                data-testid="recent-channels-scroll-area"
              >
                <div className="flex flex-col">
                  {recents.map((recent) => {
                    const displayChannelName = formatEventChannelTitle(
                      recent.channel.name,
                    );

                    return (
                      <div key={recent.channel.id} className="group">
                        <div className="flex flex-col gap-4 border-b border-border/50 px-1 py-4 transition-colors last:border-b-0 hover:bg-muted/30 sm:px-3 lg:flex-row lg:items-center lg:justify-between">
                          <div className="flex min-w-0 items-center gap-4">
                            <ChannelAvatar
                              name={displayChannelName}
                              logoUrl={recent.channel.logoUrl}
                              className="size-10"
                            />
                            <div className="flex min-w-0 flex-1 flex-col gap-1">
                              <div className="flex flex-wrap items-center gap-2">
                                <h2 className="truncate text-sm font-semibold">
                                  {displayChannelName}
                                </h2>
                                {recent.channel.categoryName ? (
                                  <Badge variant="outline">
                                    {recent.channel.categoryName}
                                  </Badge>
                                ) : null}
                                {recent.channel.isPpv ? (
                                  <Badge variant="accent">PPV</Badge>
                                ) : null}
                                {recent.channel.isFavorite ? (
                                  <Badge variant="accent">Favorite</Badge>
                                ) : null}
                                {recent.channel.isPpvFavorite ? (
                                  <Badge variant="outline">PPV saved</Badge>
                                ) : null}
                              </div>
                              <p className="text-sm text-muted-foreground">
                                Last played{" "}
                                {formatRelativeTime(recent.lastPlayedAt)} (
                                {formatDateTime(recent.lastPlayedAt)})
                              </p>
                            </div>
                          </div>
                          <div className="flex shrink-0 items-center gap-2">
                            <Button
                              variant="secondary"
                              size="sm"
                              onClick={() =>
                                favoriteMutation.mutate(recent.channel)
                              }
                              disabled={
                                favoriteMutation.isPending &&
                                favoriteMutation.variables?.id ===
                                  recent.channel.id
                              }
                            >
                              {recent.channel.isFavorite
                                ? "Unfavorite"
                                : "Favorite"}
                            </Button>
                            {recent.channel.isPpv ? (
                              <Button
                                variant="secondary"
                                size="sm"
                                onClick={() =>
                                  ppvFavoriteMutation.mutate(recent.channel)
                                }
                                disabled={
                                  ppvFavoriteMutation.isPending &&
                                  ppvFavoriteMutation.variables?.id ===
                                    recent.channel.id
                                }
                              >
                                {recent.channel.isPpvFavorite
                                  ? "Remove PPV"
                                  : "Save PPV"}
                              </Button>
                            ) : null}
                            <Button
                              size="sm"
                              onClick={() =>
                                playMutation.mutate(recent.channel.id)
                              }
                              disabled={playMutation.isPending}
                            >
                              <Play data-icon="inline-start" />
                              Play
                            </Button>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </ScrollArea>
            ) : (
              <Empty className="border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <Radio aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyTitle>No recent playback yet</EmptyTitle>
                </EmptyHeader>
              </Empty>
            )}
          </CardContent>
        </Card>

        <Card className="rounded-none border-0 border-t border-border/60 bg-transparent py-8 shadow-none sm:py-10">
          <CardHeader className="px-0 pb-6 pt-0">
            <CardTitle className="text-2xl font-semibold tracking-tight">
              Playback & appearance
            </CardTitle>
          </CardHeader>
          <CardContent className="divide-y divide-border/60 p-0">
            <div className="grid gap-4 py-6 first:pt-0 md:grid-cols-[minmax(0,240px)_minmax(0,520px)] md:items-center md:justify-between">
              <h2 className="text-sm font-medium">Theme</h2>
              <ToggleGroup
                type="single"
                value={preference}
                onValueChange={(value) => {
                  if (value) setPreference(value as ThemePreference);
                }}
                className="grid w-full grid-cols-3 rounded-lg border border-border/70 bg-muted/30 p-1"
              >
                {themeOptions.map((option) => {
                  const Icon = option.icon;
                  return (
                    <ToggleGroupItem
                      key={option.value}
                      value={option.value}
                      className="rounded-md px-3 py-2.5 data-[state=on]:bg-background data-[state=on]:shadow-sm"
                    >
                      <Icon data-icon="inline-start" />
                      {option.label}
                    </ToggleGroupItem>
                  );
                })}
              </ToggleGroup>
            </div>

            <div className="grid gap-4 py-6 md:grid-cols-[minmax(0,240px)_minmax(0,520px)] md:items-center md:justify-between">
              <h2 className="text-sm font-medium">Live playback</h2>
              <ToggleGroup
                type="single"
                value={livePlaybackPreference}
                onValueChange={(value) => {
                  if (value === "stable" || value === "low-latency") {
                    setLivePlaybackPreference(value);
                  }
                }}
                aria-label="Live playback preference"
                className="grid w-full grid-cols-2 rounded-lg border border-border/70 bg-muted/30 p-1"
              >
                <ToggleGroupItem
                  value="stable"
                  className="rounded-md px-3 py-2.5 data-[state=on]:bg-background data-[state=on]:shadow-sm"
                >
                  More stable
                </ToggleGroupItem>
                <ToggleGroupItem
                  value="low-latency"
                  className="rounded-md px-3 py-2.5 data-[state=on]:bg-background data-[state=on]:shadow-sm"
                >
                  Closer to live
                </ToggleGroupItem>
              </ToggleGroup>
            </div>

            <SettingSwitchRow
              label="Filter PPV channels by date"
              id="ppv-date-filter-toggle"
              checked={filterPpvByDate}
              onToggle={() => setFilterPpvByDate(!filterPpvByDate)}
            />

            {user?.isAdmin ? (
              <SettingSwitchRow
                title="Admin tools"
                label="Show channel moderation controls"
                id="admin-tools-toggle"
                checked={adminToolsEnabled}
                onToggle={() => setAdminToolsEnabled(!adminToolsEnabled)}
              />
            ) : null}

            <div className="grid gap-4 py-6 md:grid-cols-[minmax(0,240px)_minmax(0,520px)] md:items-center md:justify-between">
              <h2 className="text-sm font-medium">Quality channels</h2>
              <div className="flex items-center justify-between gap-4">
                <div className="flex items-center gap-2">
                  <label
                    htmlFor="quality-channels-toggle"
                    className="text-sm text-muted-foreground"
                  >
                    Only show quality channels
                  </label>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      window.location.href = "/admin";
                    }}
                  >
                    Manage
                  </Button>
                </div>
                <SettingsSwitch
                  id="quality-channels-toggle"
                  checked={qualityChannelsOnly}
                  onToggle={() => setQualityChannelsOnly(!qualityChannelsOnly)}
                />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card className="rounded-none border-0 border-t border-border/60 bg-transparent py-8 shadow-none sm:py-10">
          <CardHeader className="flex flex-row items-center justify-between gap-4 px-0 pb-6 pt-0">
            <CardTitle className="text-2xl font-semibold tracking-tight">
              Pair a screen
            </CardTitle>
            <Badge variant="outline">{remoteDevices.length} paired</Badge>
          </CardHeader>
          <CardContent className="p-0">
            <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] md:items-end">
              <Field>
                <FieldLabel htmlFor="pairingCode">Pairing code</FieldLabel>
                <Input
                  id="pairingCode"
                  value={pairCode}
                  onChange={(event) =>
                    setPairCode(event.target.value.toUpperCase().slice(0, 4))
                  }
                  placeholder="XT8P"
                  className="font-mono text-center text-lg tracking-[0.2em]"
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="receiverName">Screen name</FieldLabel>
                <Input
                  id="receiverName"
                  value={pairName}
                  onChange={(event) => setPairName(event.target.value)}
                  placeholder="Living room TV"
                />
              </Field>
              <Button
                type="button"
                size="lg"
                onClick={() =>
                  pairMutation.mutate({
                    code: pairCode,
                    rememberDevice,
                    name: pairName.trim() || undefined,
                  })
                }
                disabled={pairCode.length !== 4 || pairMutation.isPending}
              >
                Pair screen
              </Button>
            </div>
            <div className="mt-5 flex items-center gap-3">
              <SettingsSwitch
                id="remember-device-toggle"
                checked={rememberDevice}
                onToggle={() => setRememberDevice((value) => !value)}
              />
              <label
                htmlFor="remember-device-toggle"
                className="text-sm text-muted-foreground"
              >
                Remember this device
              </label>
            </div>
          </CardContent>
        </Card>
      </div>

      {user?.providerLocked ? (
        <RestrictedProviderSyncSection />
      ) : (
        <ProviderSettingsSection />
      )}

      <GoogleCalendarSettings />

      {remoteDevices.length ? (
        <Card className="rounded-none border-0 border-t border-border/60 bg-transparent py-8 shadow-none sm:py-10">
          <CardHeader className="px-0 pb-6 pt-0">
            <CardTitle className="text-2xl font-semibold tracking-tight">
              Paired receivers
            </CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-3 px-0 pb-0">
            {remoteDevices.map((device) => (
              <div
                key={device.id}
                className="flex items-center justify-between gap-3 border-b border-border/50 py-4 last:border-b-0"
              >
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="truncate text-sm font-semibold">
                      {device.name}
                    </span>
                    <Badge variant={device.online ? "accent" : "outline"}>
                      {device.online ? "Online" : "Offline"}
                    </Badge>
                    {device.currentController ? (
                      <Badge variant="accent">Current controller</Badge>
                    ) : null}
                    <Badge variant="outline">
                      {device.remembered ? "Remembered" : "Temporary"}
                    </Badge>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    {formatReceiverPlaybackSummary(device)}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => unpairMutation.mutate(device.id)}
                  disabled={unpairMutation.isPending}
                >
                  Unpair
                </Button>
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
