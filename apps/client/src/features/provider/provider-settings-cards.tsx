import type { ProviderProfile, SyncJob } from "@euripus/shared"
import { RefreshCcw } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { Separator } from "@/components/ui/separator"
import { formatDateTime, formatRelativeTime } from "@/lib/utils"

type ProviderHealthCardProps = {
  provider: ProviderProfile | null | undefined
  outputFormat: string
  playbackMode: string
  displayedEpgSourceCount: number
}

type ProviderSyncActivityCardProps = {
  latestJob: SyncJob | null | undefined
  syncProgressValue: number
  syncErrorMessage: string | null
  syncPending: boolean
  syncBlockedByActiveJob: boolean
  provider: ProviderProfile | null | undefined
  onTriggerSync: () => void
}

export function ProviderHealthCard({
  provider,
  outputFormat,
  playbackMode,
  displayedEpgSourceCount,
}: ProviderHealthCardProps) {
  return (
    <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
      <CardHeader className="px-0 pt-0 pb-4 sm:p-6 sm:pb-0">
        <CardTitle className="text-xl font-medium tracking-tight">
          Profile health
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4 px-0 pb-0 sm:p-6">
        <StatusRow
          label="Provider status"
          value={provider?.status ?? "missing"}
          detail={
            provider?.lastValidatedAt
              ? `Last validated ${formatRelativeTime(provider.lastValidatedAt)}`
              : undefined
          }
        />
        <Separator />
        <StatusRow
          label="Last sync"
          value={
            provider?.lastSyncAt ? formatRelativeTime(provider.lastSyncAt) : "Never"
          }
          detail={formatDateTime(provider?.lastSyncAt ?? null)}
        />
        <Separator />
        <StatusRow label="Output format" value={outputFormat} />
        <Separator />
        <StatusRow label="Playback routing" value={playbackMode} />
        <Separator />
        <StatusRow
          label="External EPG feeds"
          value={`${displayedEpgSourceCount}`}
        />
        {provider?.lastSyncError ? (
          <>
            <Separator />
            <div className="rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
              {provider.lastSyncError}
            </div>
          </>
        ) : null}
      </CardContent>
    </Card>
  )
}

export function ProviderSyncActivityCard({
  latestJob,
  syncProgressValue,
  syncErrorMessage,
  syncPending,
  syncBlockedByActiveJob,
  provider,
  onTriggerSync,
}: ProviderSyncActivityCardProps) {
  return (
    <Card className="rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
      <CardHeader className="px-0 pt-0 pb-4 sm:p-6 sm:pb-0">
        <CardTitle className="text-xl font-medium tracking-tight">
          Sync activity
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4 px-0 pb-0 sm:p-6">
        {latestJob ? (
          <div className="py-2 sm:rounded-2xl sm:border sm:border-border/70 sm:bg-muted/40 sm:p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="flex flex-col gap-1">
                <span className="text-sm font-medium capitalize">
                  {latestJob.currentPhase?.replaceAll("-", " ") ?? latestJob.status}
                </span>
              </div>
              <Badge
                variant={
                  latestJob.status === "failed"
                    ? "destructive"
                    : latestJob.status === "succeeded"
                      ? "accent"
                      : "outline"
                }
              >
                {latestJob.trigger}
              </Badge>
            </div>
            <Progress value={syncProgressValue} className="mt-3 h-2.5" />
          </div>
        ) : null}
        <StatusRow
          label="Latest job"
          value={latestJob?.status ?? "idle"}
          detail={
            latestJob?.createdAt
              ? `Created ${formatDateTime(latestJob.createdAt)}`
              : undefined
          }
        />
        <StatusRow
          label="Job window"
          value={latestJob?.jobType ?? "full"}
          detail={
            latestJob?.startedAt
              ? `${formatDateTime(latestJob.startedAt)} to ${formatDateTime(
                  latestJob.finishedAt ?? latestJob.startedAt,
                )}`
              : undefined
          }
        />
        {latestJob ? (
          <StatusRow
            label="Progress"
            value={`${latestJob.completedPhases}/${Math.max(latestJob.totalPhases, 0)} phases`}
            detail={latestJob.currentPhase?.replaceAll("-", " ") ?? "Queued"}
          />
        ) : null}
        {latestJob?.errorMessage ? (
          <div className="rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
            {latestJob.errorMessage}
          </div>
        ) : null}
        {syncErrorMessage ? (
          <div className="rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
            {syncErrorMessage}
          </div>
        ) : null}
        <Button
          onClick={onTriggerSync}
          disabled={syncPending || syncBlockedByActiveJob || !provider}
        >
          <RefreshCcw data-icon="inline-start" />
          {syncPending ? "Syncing..." : "Trigger full sync"}
        </Button>
      </CardContent>
    </Card>
  )
}

function StatusRow({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail?: string
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex flex-col gap-1">
        <span className="text-sm text-muted-foreground">{label}</span>
        {detail ? (
          <span className="text-xs text-muted-foreground/70">{detail}</span>
        ) : null}
      </div>
      <span className="rounded-lg border border-white/5 bg-black/20 px-3 py-1.5 text-right text-sm font-semibold capitalize tracking-tight whitespace-nowrap shadow-inner">
        {value}
      </span>
    </div>
  )
}
