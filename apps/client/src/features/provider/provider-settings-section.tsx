import type { ProviderProfile } from "@euripus/shared"
import { Plus } from "lucide-react"
import { ServerNetworkStatusCard } from "@/components/server/server-network-status-card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"
import {
  ProviderHealthCard,
  ProviderSyncActivityCard,
} from "@/features/provider/provider-settings-cards"
import { ProviderSettingsForm } from "@/features/provider/provider-settings-form"
import { useProviderSettingsForm } from "@/features/provider/use-provider-settings-form"

function formatProviderLabel(provider: ProviderProfile) {
  try {
    const url = new URL(provider.baseUrl)
    return `${provider.username} · ${url.host}`
  } catch {
    return `${provider.username} · ${provider.baseUrl}`
  }
}

export function ProviderSettingsSection() {
  const state = useProviderSettingsForm()
  const outputFormat =
    state.provider?.outputFormat?.toUpperCase() ??
    state.form.watch("outputFormat").toUpperCase()
  const playbackMode =
    state.provider?.playbackMode ?? state.form.watch("playbackMode")

  return (
    <div className="grid gap-6 xl:grid-cols-[280px_minmax(0,1.2fr)_360px]">
      <Card className="self-start rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
        <CardHeader className="px-0 pt-0 pb-4 sm:p-6 sm:pb-0">
          <div className="flex items-center justify-between gap-3">
            <CardTitle className="text-xl font-medium tracking-tight">
              Providers
            </CardTitle>
            <Badge variant="outline">{state.providers.length}</Badge>
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 px-0 pb-0 sm:p-6">
          <Button type="button" variant="secondary" onClick={state.startCreatingProvider}>
            <Plus data-icon="inline-start" />
            Add provider
          </Button>

          {state.providers.length ? (
            <div className="flex flex-col gap-2">
              {state.providers.map((provider) => {
                const isSelected =
                  !state.isCreatingProvider && state.selectedProviderId === provider.id

                return (
                  <button
                    key={provider.id}
                    type="button"
                    onClick={() => state.selectProvider(provider.id)}
                    className="flex flex-col items-start gap-1 rounded-2xl border border-border/60 bg-background/30 px-4 py-3 text-left transition-colors hover:bg-secondary/40 data-[state=active]:border-primary/40 data-[state=active]:bg-secondary/60"
                    data-state={isSelected ? "active" : "inactive"}
                  >
                    <div className="flex w-full items-center justify-between gap-3">
                      <span className="truncate text-sm font-semibold">
                        {formatProviderLabel(provider)}
                      </span>
                      <Badge
                        variant={
                          provider.status === "valid"
                            ? "accent"
                            : provider.status === "error"
                              ? "destructive"
                              : "outline"
                        }
                      >
                        {provider.status}
                      </Badge>
                    </div>
                    <span className="truncate text-xs text-muted-foreground">
                      {provider.baseUrl}
                    </span>
                  </button>
                )
              })}
            </div>
          ) : (
            <div className="rounded-2xl border border-dashed border-border/70 bg-muted/30 p-4 text-sm text-muted-foreground">
              No providers saved yet. Add your first IPTV provider to start syncing.
            </div>
          )}
        </CardContent>
      </Card>

      <Separator className="xl:hidden" />

      <ProviderSettingsForm
        form={state.form}
        provider={state.provider}
        isCreatingProvider={state.isCreatingProvider}
        latestJob={state.latestJob}
        savePending={state.saveMutation.isPending}
        validatePending={state.validateMutation.isPending}
        deletePending={state.deleteMutation.isPending}
        validationMessage={state.feedbackMessage ?? undefined}
        onSubmit={state.submitSave}
        onValidate={state.submitValidate}
        onDelete={state.submitDelete}
      />

      <Separator className="sm:hidden" />

      <div className="flex flex-col gap-6">
        <ProviderHealthCard
          provider={state.provider}
          outputFormat={outputFormat}
          playbackMode={playbackMode}
          displayedEpgSourceCount={state.displayedEpgSourceCount}
        />

        <Separator className="sm:hidden" />

        <ServerNetworkStatusCard />

        <Separator className="sm:hidden" />

        <ProviderSyncActivityCard
          latestJob={state.latestJob}
          syncProgressValue={state.syncProgressValue}
          syncErrorMessage={state.syncErrorMessage}
          syncPending={state.syncMutation.isPending}
          syncBlockedByActiveJob={state.syncBlockedByActiveJob}
          provider={state.provider}
          onTriggerSync={() => state.syncMutation.mutate()}
        />
      </div>
    </div>
  )
}
