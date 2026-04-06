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
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderSettingsSection } from "@/features/provider/provider-settings-section";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { useChannelPlaybackMutation } from "@/hooks/use-playback-actions";
import {
  getProvider,
  getRecents,
  getRemoteReceivers,
  pairReceiver,
  unpairReceiver,
} from "@/lib/api";
import { formatDateTime, formatRelativeTime } from "@/lib/utils";
import type { ThemePreference } from "@/store/theme-store";
import { useThemeStore } from "@/store/theme-store";

const themeOptions: Array<{
  value: ThemePreference;
  label: string;
  icon: typeof LaptopMinimal;
}> = [
  { value: "system", label: "System", icon: LaptopMinimal },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
] as const;

export function SettingsPage() {
  const queryClient = useQueryClient();
  const [pairCode, setPairCode] = useState("");
  const [rememberDevice, setRememberDevice] = useState(true);
  const [pairName, setPairName] = useState("");
  const recentsQuery = useQuery({ queryKey: ["recents"], queryFn: getRecents });
  const providerQuery = useQuery({
    queryKey: ["provider"],
    queryFn: getProvider,
  });
  const remoteDevicesQuery = useQuery({
    queryKey: ["remote", "receivers"],
    queryFn: getRemoteReceivers,
  });
  const favoriteMutation = useChannelFavoriteMutation();
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
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);
  const setPreference = useThemeStore((state) => state.setPreference);
  const recents = recentsQuery.data ?? [];
  const provider = providerQuery.data;
  const remoteDevices = remoteDevicesQuery.data ?? [];
  const playMutation = useChannelPlaybackMutation();

  return (
    <div className="flex flex-col gap-5 sm:gap-6">
      <PageHeader
        title="Settings"
        meta={
          <>
            <Badge variant="accent">{resolvedTheme} theme</Badge>
            <Badge variant="outline">
              {remoteDevices.length} paired screens
            </Badge>
            <Badge variant="outline">{recents.length} recent channels</Badge>
            <Badge variant="outline">
              {provider?.status ?? "provider missing"}
            </Badge>
          </>
        }
      />

      <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
        <Card className="self-start rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-5 sm:pb-0">
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
                  <ToggleGroupItem
                    key={option.value}
                    value={option.value}
                    className="justify-start rounded-xl px-3 py-2.5"
                  >
                    <Icon data-icon="inline-start" />
                    {option.label}
                  </ToggleGroupItem>
                );
              })}
            </ToggleGroup>

            <Separator />

            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between gap-3">
                <span className="text-sm font-medium">Pair screen</span>
                <Badge variant="outline">{remoteDevices.length} paired</Badge>
              </div>
              <p className="text-sm text-muted-foreground">
                Enter the 4-character code shown on the receiver screen.
              </p>
              <Input
                aria-label="Pairing code"
                value={pairCode}
                onChange={(event) =>
                  setPairCode(event.target.value.toUpperCase().slice(0, 4))
                }
                placeholder="XT8P"
              />
              <Input
                aria-label="Receiver name"
                value={pairName}
                onChange={(event) => setPairName(event.target.value)}
                placeholder="Living room TV"
              />
              <div className="flex items-center justify-between gap-3 rounded-lg border border-border/70 bg-muted/20 px-4 py-3">
                <label
                  htmlFor="remember-device-toggle"
                  className="text-sm font-medium"
                >
                  Remember this device
                </label>
                <button
                  id="remember-device-toggle"
                  type="button"
                  role="switch"
                  aria-checked={rememberDevice}
                  aria-label="Remember this device"
                  onClick={() => setRememberDevice((value) => !value)}
                  className="relative inline-flex h-8 w-14 shrink-0 items-center rounded-full border border-white/10 bg-muted transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
                  data-state={rememberDevice ? "checked" : "unchecked"}
                >
                  <span
                    className="absolute inset-0 rounded-full bg-gradient-to-r from-violet-500 to-fuchsia-400 opacity-0 transition-opacity data-[state=checked]:opacity-100"
                    data-state={rememberDevice ? "checked" : "unchecked"}
                  />
                  <span
                    className="relative z-10 mx-1 block size-6 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-6"
                    data-state={rememberDevice ? "checked" : "unchecked"}
                  />
                </button>
              </div>
              <Button
                type="button"
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
          </CardContent>
        </Card>

        <Separator className="sm:hidden" />

        <Card className="overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="flex flex-row items-center justify-between gap-4 px-0 pb-4 pt-0 sm:p-5 sm:pb-0">
            <CardTitle>Recent channels</CardTitle>
            <Badge variant="outline">{recents.length}</Badge>
          </CardHeader>
          <CardContent className="p-0">
            {recents.length ? (
              <ScrollArea
                className="h-[24rem] sm:h-[26rem]"
                data-testid="recent-channels-scroll-area"
              >
                <div className="flex flex-col">
                  {recents.map((recent, index) => (
                    <div key={recent.channel.id}>
                      {index > 0 ? <Separator /> : null}
                      <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                        <div className="flex min-w-0 items-center gap-4">
                          <ChannelAvatar
                            name={recent.channel.name}
                            logoUrl={recent.channel.logoUrl}
                            className="size-10"
                          />
                          <div className="flex min-w-0 flex-1 flex-col gap-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <h2 className="truncate text-sm font-semibold">
                                {recent.channel.name}
                              </h2>
                              {recent.channel.categoryName ? (
                                <Badge variant="outline">
                                  {recent.channel.categoryName}
                                </Badge>
                              ) : null}
                              {recent.channel.isFavorite ? (
                                <Badge variant="accent">Favorite</Badge>
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

      {remoteDevices.length ? (
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-5 sm:pb-0">
            <CardTitle>Paired receivers</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-3 px-0 pb-0 sm:p-5">
            {remoteDevices.map((device) => (
              <div
                key={device.id}
                className="flex items-center justify-between gap-3 rounded-xl border border-border/70 p-4"
              >
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="truncate text-sm font-semibold">
                      {device.name}
                    </span>
                    <Badge variant={device.online ? "accent" : "outline"}>
                      {device.online ? "Online" : "Offline"}
                    </Badge>
                    <Badge variant="outline">
                      {device.remembered ? "Remembered" : "Temporary"}
                    </Badge>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    {device.currentPlayback
                      ? `Now playing ${device.currentPlayback.title}`
                      : device.platform}
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
