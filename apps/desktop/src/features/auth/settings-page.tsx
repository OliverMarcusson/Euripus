import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LaptopMinimal, Moon, Radio, Sun, Trash2 } from "lucide-react";
import { PageHeader } from "@/components/layout/page-header";
import { ChannelAvatar } from "@/components/ui/channel-avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { getRecents, getSessions, revokeSession } from "@/lib/api";
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
  const sessionsQuery = useQuery({ queryKey: ["sessions"], queryFn: getSessions });
  const recentsQuery = useQuery({ queryKey: ["recents"], queryFn: getRecents });
  const revokeMutation = useMutation({
    mutationFn: revokeSession,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
  });
  const preference = useThemeStore((state) => state.preference);
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);
  const setPreference = useThemeStore((state) => state.setPreference);
  const sessions = sessionsQuery.data ?? [];
  const recents = recentsQuery.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Settings"
        description="Control appearance, inspect active sessions, and keep an eye on the channels you returned to most recently."
        meta={
          <>
            <Badge variant="accent">{resolvedTheme} theme</Badge>
            <Badge variant="outline">{sessions.length} sessions</Badge>
            <Badge variant="outline">{recents.length} recent channels</Badge>
          </>
        }
      />

      <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
        <Card className="self-start">
          <CardHeader className="pb-4">
            <CardTitle>Appearance</CardTitle>
            <CardDescription>Choose how Euripus should look on this device.</CardDescription>
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
            <p className="text-sm text-muted-foreground">
              {preference === "system"
                ? `Euripus is following your ${resolvedTheme} system theme.`
                : `Euripus is locked to ${resolvedTheme} mode until you change it.`}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Recent channels</CardTitle>
            <CardDescription>Server-side recents can power a faster return path across surfaces.</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            {recents.length ? (
              recents.map((recent, index) => (
                <div key={recent.channel.id}>
                  {index > 0 ? <Separator /> : null}
                  <div className="flex items-center gap-4 p-5">
                    <ChannelAvatar name={recent.channel.name} logoUrl={recent.channel.logoUrl} className="size-10" />
                    <div className="flex min-w-0 flex-1 flex-col gap-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <h2 className="truncate text-sm font-semibold">{recent.channel.name}</h2>
                        {recent.channel.categoryName ? <Badge variant="outline">{recent.channel.categoryName}</Badge> : null}
                      </div>
                      <p className="text-sm text-muted-foreground">
                        Last played {formatRelativeTime(recent.lastPlayedAt)} ({formatDateTime(recent.lastPlayedAt)})
                      </p>
                    </div>
                  </div>
                </div>
              ))
            ) : (
              <Empty className="border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <Radio aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyTitle>No recent playback yet</EmptyTitle>
                  <EmptyDescription>Start a channel from the guide, favorites, or search to populate this list.</EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Sessions</CardTitle>
          <CardDescription>Review active refresh-token sessions managed by the server.</CardDescription>
        </CardHeader>
        <CardContent className="p-0">
          {sessions.length ? (
            sessions.map((session, index) => (
              <div key={session.id}>
                {index > 0 ? <Separator /> : null}
                <div className="flex flex-col gap-4 p-5 lg:flex-row lg:items-center lg:justify-between">
                  <div className="flex min-w-0 flex-1 flex-col gap-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <h2 className="text-base font-semibold">{session.current ? "Current device" : "Signed-in device"}</h2>
                      {session.current ? <Badge variant="accent">Current</Badge> : <Badge variant="outline">Active</Badge>}
                    </div>
                    <p className="text-sm text-muted-foreground">{session.userAgent ?? "Unknown device details"}</p>
                    <div className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
                      <span>Created {formatDateTime(session.createdAt)}</span>
                      <span>Last used {formatRelativeTime(session.lastUsedAt ?? session.createdAt)}</span>
                      <span>Expires {formatDateTime(session.expiresAt)}</span>
                    </div>
                  </div>
                  {!session.current ? (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => revokeMutation.mutate(session.id)}
                      disabled={revokeMutation.isPending}
                    >
                      <Trash2 data-icon="inline-start" />
                      Revoke
                    </Button>
                  ) : null}
                </div>
              </div>
            ))
          ) : (
            <Empty className="border-0">
              <EmptyHeader>
                <EmptyTitle>No sessions found</EmptyTitle>
                <EmptyDescription>Your account has not established any refresh-token sessions yet.</EmptyDescription>
              </EmptyHeader>
            </Empty>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
