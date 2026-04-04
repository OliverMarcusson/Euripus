import { useQuery } from "@tanstack/react-query";
import { LaptopMinimal, Moon, Sun } from "lucide-react";
import { getSessions, getRecents } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { formatDateTime } from "@/lib/utils";
import type { ThemePreference } from "@/store/theme-store";
import { useThemeStore } from "@/store/theme-store";

const themeOptions: Array<{
  value: ThemePreference;
  label: string;
  icon: typeof LaptopMinimal;
}> = [
  {
    value: "system",
    label: "System",
    icon: LaptopMinimal,
  },
  {
    value: "light",
    label: "Light",
    icon: Sun,
  },
  {
    value: "dark",
    label: "Dark",
    icon: Moon,
  },
];

export function SettingsPage() {
  const sessionsQuery = useQuery({ queryKey: ["sessions"], queryFn: getSessions });
  const recentsQuery = useQuery({ queryKey: ["recents"], queryFn: getRecents });
  const preference = useThemeStore((state) => state.preference);
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);
  const setPreference = useThemeStore((state) => state.setPreference);

  return (
    <div className="grid gap-6 xl:grid-cols-2">
      <Card className="xl:col-span-2">
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
          <CardDescription>Choose how Euripus should look on this device.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <ToggleGroup
            type="single"
            value={preference}
            onValueChange={(v) => { if (v) setPreference(v as ThemePreference); }}
            className="flex flex-wrap gap-2 rounded-xl bg-secondary/60 p-1"
          >
            {themeOptions.map((option) => {
              const Icon = option.icon;
              return (
                <ToggleGroupItem
                  key={option.value}
                  value={option.value}
                  className="min-w-32 flex-1 justify-start"
                >
                  <Icon />
                  {option.label}
                </ToggleGroupItem>
              );
            })}
          </ToggleGroup>
          <div className="rounded-xl border border-border bg-card/60 p-4">
            <p className="font-medium">Current behavior</p>
            <p className="mt-1 text-sm text-muted-foreground">
              {preference === "system"
                ? `Euripus is following your ${resolvedTheme} system theme.`
                : `Euripus is locked to ${resolvedTheme} mode until you change it.`}
            </p>
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Sessions</CardTitle>
          <CardDescription>Active refresh-token sessions managed by the server.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {(sessionsQuery.data ?? []).map((session) => (
            <div key={session.id} className="rounded-xl border border-border bg-card/60 p-4">
              <p className="font-medium">{session.current ? "Current device" : "Other device"}</p>
              <p className="text-sm text-muted-foreground">Created: {formatDateTime(session.createdAt)}</p>
              <p className="text-sm text-muted-foreground">Expires: {formatDateTime(session.expiresAt)}</p>
            </div>
          ))}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Recent Channels</CardTitle>
          <CardDescription>Server-side recents can seed a future Android TV home screen.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {(recentsQuery.data ?? []).map((recent) => (
            <div key={recent.channel.id} className="rounded-xl border border-border bg-card/60 p-4">
              <p className="font-medium">{recent.channel.name}</p>
              <p className="text-sm text-muted-foreground">Last played: {formatDateTime(recent.lastPlayedAt)}</p>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
