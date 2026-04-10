import { useQuery } from "@tanstack/react-query";
import { Globe, ShieldCheck, ShieldOff, Server } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getServerNetworkStatus } from "@/lib/api";
import { SERVER_NETWORK_STALE_TIME_MS } from "@/lib/query-cache";
import { cn, formatDateTime, formatRelativeTime } from "@/lib/utils";

type ServerNetworkStatusCardProps = {
  className?: string;
};

export function ServerNetworkStatusCard({
  className,
}: ServerNetworkStatusCardProps) {
  const statusQuery = useQuery({
    queryKey: ["server-network-status"],
    queryFn: getServerNetworkStatus,
    refetchInterval: 60_000,
    retry: 1,
    staleTime: SERVER_NETWORK_STALE_TIME_MS,
  });
  const status = statusQuery.data;
  const vpnLabel = status?.vpnProvider ? `${status.vpnProvider} active` : "VPN active";

  return (
    <Card className={cn("overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl", className)}>
      <CardHeader className="flex flex-row items-start justify-between gap-4 px-0 pt-0 pb-4 sm:p-6 sm:pb-0">
        <CardTitle className="shrink-0 text-xl font-medium tracking-tight whitespace-nowrap">Server route</CardTitle>
        <div className="flex flex-wrap justify-end gap-2">
          <Badge variant={statusQuery.isError ? "destructive" : statusQuery.isSuccess ? "accent" : "outline"} className="gap-1.5">
            <Server className="size-3.5" />
            {statusQuery.isError ? "Server unreachable" : statusQuery.isSuccess ? "Server online" : "Checking server"}
          </Badge>
          <Badge variant={status?.vpnActive ? "success" : "outline"} className="gap-1.5">
            {status?.vpnActive ? <ShieldCheck className="size-3.5" /> : <ShieldOff className="size-3.5" />}
            {status?.vpnActive ? vpnLabel : "VPN off"}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-4 px-0 pb-0 sm:p-6">
        <StatusRow
          label="Server"
          value={statusQuery.isError ? "Offline" : statusQuery.isSuccess ? "Online" : "Checking"}
        />
        <Separator />
        <StatusRow
          label="VPN routing"
          value={
            statusQuery.isSuccess
              ? status?.vpnActive
                ? status.vpnProvider ?? "Enabled"
                : "Disabled"
              : "Unknown"
          }
          capitalizeValue={false}
        />
        <Separator />
        <StatusRow
          label="Public IP"
          value={status?.publicIp ?? (statusQuery.isError ? "Unavailable" : "Looking up...")}
          detail={
            status?.publicIp
              ? `Checked ${formatRelativeTime(status.publicIpCheckedAt)} (${formatDateTime(status.publicIpCheckedAt)})`
              : status?.publicIpError
          }
          capitalizeValue={false}
        />
        {status?.publicIp ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Globe className="size-4" />
            <span>tv.olivermarcusson.se</span>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function StatusRow({
  label,
  value,
  detail,
  capitalizeValue = true,
}: {
  label: string;
  value: string;
  detail?: string | null;
  capitalizeValue?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex flex-col gap-1">
        <span className="text-sm text-muted-foreground">{label}</span>
        {detail ? <span className="text-xs text-muted-foreground/70">{detail}</span> : null}
      </div>
      <span className={cn("text-sm font-semibold tracking-tight text-right whitespace-nowrap bg-black/20 px-3 py-1.5 rounded-lg border border-white/5 shadow-inner", capitalizeValue && "capitalize")}>
        {value}
      </span>
    </div>
  );
}
