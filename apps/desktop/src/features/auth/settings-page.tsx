import { useQuery } from "@tanstack/react-query";
import { getSessions, getRecents } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { formatDateTime } from "@/lib/utils";

export function SettingsPage() {
  const sessionsQuery = useQuery({ queryKey: ["sessions"], queryFn: getSessions });
  const recentsQuery = useQuery({ queryKey: ["recents"], queryFn: getRecents });

  return (
    <div className="grid gap-6 xl:grid-cols-2">
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

