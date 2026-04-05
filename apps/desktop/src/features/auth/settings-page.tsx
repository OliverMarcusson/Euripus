import { useQuery } from "@tanstack/react-query";
import { LaptopMinimal, Moon, Play, Radio, Sun } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderSettingsSection } from "@/features/provider/provider-settings-section";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";
import { getProvider, getRecents, getRemoteDevices } from "@/lib/api";
import { formatDateTime, formatRelativeTime } from "@/lib/utils";
import { usePlaybackDeviceStore } from "@/store/playback-device-store";
import type { ThemePreference } from "@/store/theme-store";
import { useThemeStore } from "@/store/theme-store";
import type { TvModePreference } from "@/store/tv-mode-store";
import { useTvModeStore } from "@/store/tv-mode-store";

const themeOptions: Array<{
  value: ThemePreference;
  label: string;
  icon: typeof LaptopMinimal;
}> = [
  { value: "system", label: "System", icon: LaptopMinimal },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
] as const;

const tvModeOptions: Array<{
  value: TvModePreference;
  label: string;
}> = [
  { value: "auto", label: "Auto" },
  { value: "on", label: "TV mode" },
  { value: "off", label: "Desktop" },
] as const;

export function SettingsPage() {
  const recentsQuery = useQuery({ queryKey: ["recents"], queryFn: getRecents });
  const providerQuery = useQuery({ queryKey: ["provider"], queryFn: getProvider });
  const remoteDevicesQuery = useQuery({ queryKey: ["remote", "devices"], queryFn: getRemoteDevices });
  const favoriteMutation = useChannelFavoriteMutation();
  const preference = useThemeStore((state) => state.preference);
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);
  const setPreference = useThemeStore((state) => state.setPreference);
  const tvModePreference = useTvModeStore((state) => state.preference);
  const isTvMode = useTvModeStore((state) => state.isTvMode);
  const setTvModePreference = useTvModeStore((state) => state.setPreference);
  const activeDeviceId = usePlaybackDeviceStore((state) => state.activeDeviceId);
  const deviceName = usePlaybackDeviceStore((state) => state.name);
  const remoteTargetEnabled = usePlaybackDeviceStore((state) => state.remoteTargetEnabled);
  const setDeviceName = usePlaybackDeviceStore((state) => state.setName);
  const setRemoteTargetEnabled = usePlaybackDeviceStore((state) => state.setRemoteTargetEnabled);
  const recents = recentsQuery.data ?? [];
  const provider = providerQuery.data;
  const playMutation = useChannelPlaybackMutation();
  const activeRemoteDevice = (remoteDevicesQuery.data ?? []).find((device) => device.id === activeDeviceId);
  const remoteTargetStatus = !remoteTargetEnabled
    ? "Target off"
    : activeRemoteDevice?.currentController
      ? "Controlled now"
      : activeRemoteDevice?.online
        ? "Target online"
        : "Target offline";

  return (
    <div className="flex flex-col gap-5 sm:gap-6">
      <PageHeader
        title="Settings"
        meta={
          <>
            <Badge variant="accent">{resolvedTheme} theme</Badge>
            <Badge variant="outline">{isTvMode ? "TV mode on" : "TV mode off"}</Badge>
            <Badge variant="outline">{remoteTargetStatus}</Badge>
            <Badge variant="outline">{recents.length} recent channels</Badge>
            <Badge variant="outline">{provider?.status ?? "provider missing"}</Badge>
          </>
        }
      />

      <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
        <Card className="self-start rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="px-0 pt-0 pb-4 sm:p-5 sm:pb-0">
            <CardTitle>Appearance</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4 px-0 pb-0 sm:p-5">
            <ToggleGroup
              type="single"
              value={preference}
              onValueChange={(value) => {
                if (value) {
                  setPreference(value as ThemePreference);
                }
              }}
              className="grid w-full grid-cols-1 gap-2 rounded-2xl bg-secondary/60 p-2"
            >
              {themeOptions.map((option) => {
                const Icon = option.icon;

                return (
                  <ToggleGroupItem key={option.value} value={option.value} className="justify-start rounded-xl px-3 py-2.5">
                    <Icon data-icon="inline-start" />
                    {option.label}
                  </ToggleGroupItem>
                );
              })}
            </ToggleGroup>

            <Separator />

            <div className="flex flex-col gap-3">
              <span className="text-sm font-medium">Remote-friendly UI</span>
              <ToggleGroup
                type="single"
                value={tvModePreference}
                onValueChange={(value) => {
                  if (value) {
                    setTvModePreference(value as TvModePreference);
                  }
                }}
                className="grid w-full grid-cols-1 gap-2 rounded-2xl bg-secondary/60 p-2"
              >
                {tvModeOptions.map((option) => (
                  <ToggleGroupItem key={option.value} value={option.value} className="justify-start rounded-xl px-3 py-2.5">
                    {option.label}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            </div>

            <Separator />

            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between gap-3">
                <span className="text-sm font-medium">Remote playback target</span>
                <Badge variant={remoteTargetEnabled ? "accent" : "outline"}>{remoteTargetStatus}</Badge>
              </div>
              <p className="text-sm text-muted-foreground">
                Enable this on the screen you want to control from your phone. Once enabled, the device advertises
                itself automatically while you are signed in.
              </p>
              <Input
                aria-label="Device name"
                value={deviceName}
                onChange={(event) => setDeviceName(event.target.value)}
                placeholder="Living room TV"
              />
              <Button
                type="button"
                variant={remoteTargetEnabled ? "secondary" : "default"}
                onClick={() => setRemoteTargetEnabled(!remoteTargetEnabled)}
              >
                {remoteTargetEnabled ? "Disable target mode" : "Use this device as a playback target"}
              </Button>
            </div>
          </CardContent>
        </Card>

        <Separator className="sm:hidden" />

        <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="flex flex-row items-center justify-between gap-4 px-0 pt-0 pb-4 sm:p-5 sm:pb-0">
            <CardTitle>Recent channels</CardTitle>
            <Badge variant="outline">{recents.length}</Badge>
          </CardHeader>
          <CardContent className="p-0">
            {recents.length ? (
              <ScrollArea className="h-[24rem] sm:h-[26rem]" data-testid="recent-channels-scroll-area">
                <div className="flex flex-col">
                  {recents.map((recent, index) => (
                    <div key={recent.channel.id}>
                      {index > 0 ? <Separator /> : null}
                      <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                        <div className="flex min-w-0 items-center gap-4">
                          <ChannelAvatar name={recent.channel.name} logoUrl={recent.channel.logoUrl} className="size-10" />
                          <div className="flex min-w-0 flex-1 flex-col gap-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <h2 className="truncate text-sm font-semibold">{recent.channel.name}</h2>
                              {recent.channel.categoryName ? <Badge variant="outline">{recent.channel.categoryName}</Badge> : null}
                              {recent.channel.isFavorite ? <Badge variant="accent">Favorite</Badge> : null}
                            </div>
                            <p className="text-sm text-muted-foreground">
                              Last played {formatRelativeTime(recent.lastPlayedAt)} ({formatDateTime(recent.lastPlayedAt)})
                            </p>
                          </div>
                        </div>
                        <div className="flex shrink-0 items-center gap-2">
                          <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => favoriteMutation.mutate(recent.channel)}
                            disabled={favoriteMutation.isPending && favoriteMutation.variables?.id === recent.channel.id}
                          >
                            {recent.channel.isFavorite ? "Unfavorite" : "Favorite"}
                          </Button>
                          <Button
                            size="sm"
                            onClick={() => playMutation.mutate(recent.channel.id)}
                            disabled={playMutation.isPending}
                          >
                            <Play data-icon="inline-start" />
                            Play
                          </Button>
                        </div>
                      </div>
                    </div>
                  ))}
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
      </div>

      <Separator className="sm:hidden" />

      <ProviderSettingsSection />
    </div>
  );
}
