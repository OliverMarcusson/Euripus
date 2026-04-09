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
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
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
import { formatReceiverPlaybackSummary } from "@/lib/receiver-playback";
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
        <Card className="self-start rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-6 sm:pb-0">
            <CardTitle className="text-xl font-medium tracking-tight">Appearance</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-6 px-0 pb-0 sm:p-6">
            <ToggleGroup
              type="single"
              value={preference}
              onValueChange={(value) => {
                if (value) {
                  setPreference(value as ThemePreference);
                }
              }}
              className="grid w-full grid-cols-1 gap-1.5 rounded-2xl bg-black/20 p-1.5 shadow-inner"
            >
              {themeOptions.map((option) => {
                const Icon = option.icon;

                return (
                  <ToggleGroupItem
                    key={option.value}
                    value={option.value}
                    className="justify-start rounded-xl px-4 py-3 data-[state=on]:bg-secondary/80 data-[state=on]:shadow-sm transition-all"
                  >
                    <Icon data-icon="inline-start" />
                    {option.label}
                  </ToggleGroupItem>
                );
              })}
            </ToggleGroup>

            <Separator />

            <div className="flex flex-col gap-5">
              <div className="flex items-center justify-between gap-3">
                <span className="text-base font-medium tracking-tight">Pair screen</span>
                <Badge variant="outline" className="bg-background/50">{remoteDevices.length} paired</Badge>
              </div>
              <p className="text-sm text-muted-foreground leading-relaxed">
                Enter the 4-character code shown on the receiver screen.
              </p>
              
              <FieldGroup className="gap-4">
                <Field>
                  <FieldLabel className="sr-only">Pairing code</FieldLabel>
                  <Input
                    aria-label="Pairing code"
                    value={pairCode}
                    onChange={(event) =>
                      setPairCode(event.target.value.toUpperCase().slice(0, 4))
                    }
                    placeholder="XT8P"
                    className="bg-background/50 font-mono text-center text-lg tracking-[0.2em]"
                  />
                </Field>
                <Field>
                  <FieldLabel className="sr-only">Receiver name</FieldLabel>
                  <Input
                    aria-label="Receiver name"
                    value={pairName}
                    onChange={(event) => setPairName(event.target.value)}
                    placeholder="Living room TV"
                    className="bg-background/50"
                  />
                </Field>
              </FieldGroup>
              
              <div className="flex items-center justify-between gap-3 rounded-2xl border border-border/40 bg-black/10 px-4 py-3 shadow-inner">
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
                  className="relative inline-flex h-8 w-14 shrink-0 items-center rounded-full border border-white/10 bg-muted/50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background data-[state=checked]:bg-primary"
                  data-state={rememberDevice ? "checked" : "unchecked"}
                >
                  <span
                    className="relative z-10 mx-1 block size-6 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-6"
                    data-state={rememberDevice ? "checked" : "unchecked"}
                  />
                </button>
              </div>
              
              <Button
                type="button"
                size="lg"
                className="mt-1 rounded-xl shadow-lg shadow-primary/20"
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

        <Card className="flex flex-col overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
          <CardHeader className="flex shrink-0 flex-row items-center justify-between gap-4 px-0 pb-4 pt-0 sm:p-6 sm:pb-0">
            <CardTitle className="text-xl font-medium tracking-tight">Recent channels</CardTitle>
            <Badge variant="outline" className="bg-background/60 backdrop-blur-md">{recents.length}</Badge>
          </CardHeader>
          <CardContent className="flex flex-col p-0 pb-2 sm:px-4 sm:pb-4">
            {recents.length ? (
              <ScrollArea
                className="h-[22rem] sm:h-[26rem] xl:h-[33rem] pr-3"
                data-testid="recent-channels-scroll-area"
              >
                <div className="flex flex-col gap-1.5 py-2">
                  {recents.map((recent, index) => (
                    <div key={recent.channel.id} className="group">
                      <div className="flex flex-col gap-4 p-4 rounded-2xl transition-all duration-300 hover:bg-secondary/40 hover:shadow-md lg:flex-row lg:items-center lg:justify-between">
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
                        <div className="flex shrink-0 items-center gap-2 opacity-100 transition-opacity duration-300 lg:opacity-0 lg:group-hover:opacity-100">
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
        <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
          <CardHeader className="px-0 pb-4 pt-0 sm:p-6 sm:pb-0">
            <CardTitle className="text-xl font-medium tracking-tight">Paired receivers</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-3 px-0 pb-0 sm:p-6">
            {remoteDevices.map((device) => (
              <div
                key={device.id}
                className="flex items-center justify-between gap-3 rounded-2xl border border-border/40 bg-background/30 p-5 shadow-sm backdrop-blur-md transition-colors hover:bg-secondary/40"
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
