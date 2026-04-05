import { useQuery } from "@tanstack/react-query";
import { Globe, ShieldCheck, ShieldOff, Server } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getServerNetworkStatus } from "@/lib/api";
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
  });
  const status = statusQuery.data;
  const vpnLabel = status?.vpnProvider ? `${status.vpnProvider} active` : "VPN active";

  return (
    <Card className={cn("overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-xl sm:border sm:bg-card sm:shadow-sm", className)}>
      <CardHeader className="flex flex-row items-start justify-between gap-4 px-0 pt-0 pb-4 sm:p-5 sm:pb-0">
        <CardTitle>Server route</CardTitle>
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={statusQuery.isError ? "destructive" : statusQuery.isSuccess ? "accent" : "outline"}>
            <Server data-icon="inline-start" />
            {statusQuery.isError ? "Server unreachable" : statusQuery.isSuccess ? "Server online" : "Checking server"}
          </Badge>
          <Badge variant={status?.vpnActive ? "success" : "outline"}>
            {status?.vpnActive ? <ShieldCheck data-icon="inline-start" /> : <ShieldOff data-icon="inline-start" />}
            {status?.vpnActive ? vpnLabel : "VPN off"}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-4 px-0 pb-0 sm:p-5">
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
    <div className="flex flex-col gap-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className={cn("text-base font-semibold", capitalizeValue && "capitalize")}>{value}</span>
      {detail ? <span className="text-sm text-muted-foreground">{detail}</span> : null}
    </div>
  );
}
