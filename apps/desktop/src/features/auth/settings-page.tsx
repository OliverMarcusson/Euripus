import { useQuery } from "@tanstack/react-query";
import { LaptopMinimal, Moon, Radio, Sun } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderSettingsSection } from "@/features/provider/provider-settings-section";
import { useChannelFavoriteMutation } from "@/hooks/use-channel-favorite";
import { getProvider, getRecents } from "@/lib/api";
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
  const recentsQuery = useQuery({ queryKey: ["recents"], queryFn: getRecents });
  const providerQuery = useQuery({ queryKey: ["provider"], queryFn: getProvider });
  const favoriteMutation = useChannelFavoriteMutation();
  const preference = useThemeStore((state) => state.preference);
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);
  const setPreference = useThemeStore((state) => state.setPreference);
  const recents = recentsQuery.data ?? [];
  const provider = providerQuery.data;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Settings"
        meta={
          <>
            <Badge variant="accent">{resolvedTheme} theme</Badge>
            <Badge variant="outline">{recents.length} recent channels</Badge>
            <Badge variant="outline">{provider?.status ?? "provider missing"}</Badge>
          </>
        }
      />

      <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
        <Card className="self-start">
          <CardHeader>
            <CardTitle>Appearance</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
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
          </CardContent>
        </Card>

        <Card className="overflow-hidden">
          <CardHeader className="flex flex-row items-center justify-between gap-4">
            <CardTitle>Recent channels</CardTitle>
            <Badge variant="outline">{recents.length}</Badge>
          </CardHeader>
          <CardContent className="p-0">
            {recents.length ? (
              <ScrollArea className="max-h-[26rem]" data-testid="recent-channels-scroll-area">
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

      <ProviderSettingsSection />
    </div>
  );
}
