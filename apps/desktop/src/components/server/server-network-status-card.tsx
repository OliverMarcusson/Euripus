import { useQuery } from "@tanstack/react-query";
import { Globe, ShieldCheck, ShieldOff, Server } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getServerNetworkStatus } from "@/lib/api";
import { cn, formatDateTime, formatRelativeTime } from "@/lib/utils";

type ServerNetworkStatusCardProps = {
  className?: string;
  description?: string;
};

const DEFAULT_DESCRIPTION = "Quick check for whether this Euripus server is online, which public IP it is using, and whether VPN routing is enabled.";

export function ServerNetworkStatusCard({
  className,
  description = DEFAULT_DESCRIPTION,
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
    <Card className={cn("overflow-hidden", className)}>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div className="flex flex-col gap-1">
          <CardTitle>Server route</CardTitle>
          <CardDescription>{description}</CardDescription>
        </div>
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
      <CardContent className="flex flex-col gap-4">
        <StatusRow
          label="Server"
          value={statusQuery.isError ? "Offline" : statusQuery.isSuccess ? "Online" : "Checking"}
          detail={
            statusQuery.isError
              ? "The web client could not reach the Euripus API."
              : statusQuery.isSuccess
                ? "The Euripus API responded successfully."
                : "Checking the Euripus API from this client."
          }
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
          detail={
            statusQuery.isSuccess
              ? status?.vpnActive
                ? "Outbound server traffic is configured to use VPN routing."
                : "Outbound server traffic is not using the VPN override."
              : "VPN status becomes available after the server responds."
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
              : status?.publicIpError ?? "This is the outbound IP the Euripus server is currently using."
          }
          capitalizeValue={false}
        />
        <div className="rounded-2xl border border-border/70 bg-muted/40 p-4 text-sm text-muted-foreground">
          <div className="flex items-center gap-2 font-medium text-foreground">
            <Globe className="size-4 text-muted-foreground" />
            Useful for `tv.olivermarcusson.se`
          </div>
          <p className="mt-2">
            Open this page to confirm the API is reachable and verify that the server egress IP matches the VPN route you expect.
          </p>
        </div>
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
  detail: string;
  capitalizeValue?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className={cn("text-base font-semibold", capitalizeValue && "capitalize")}>{value}</span>
      <span className="text-sm text-muted-foreground">{detail}</span>
    </div>
  );
}
