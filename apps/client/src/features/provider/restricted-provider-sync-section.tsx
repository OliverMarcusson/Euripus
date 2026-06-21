import { ProviderSyncActivityCard } from "@/features/provider/provider-settings-cards"
import { useProviderSettingsForm } from "@/features/provider/use-provider-settings-form"

export function RestrictedProviderSyncSection() {
  const state = useProviderSettingsForm()

  return (
    <section className="flex flex-col gap-4">
      <div>
        <h2 className="text-xl font-medium tracking-tight">Provider sync</h2>
        <p className="text-sm text-muted-foreground">
          Your provider is managed by an administrator. You can refresh its catalog here.
        </p>
      </div>
      <ProviderSyncActivityCard
        latestJob={state.latestJob}
        syncProgressValue={state.syncProgressValue}
        syncErrorMessage={state.syncErrorMessage}
        syncPending={state.syncMutation.isPending}
        syncBlockedByActiveJob={state.syncBlockedByActiveJob}
        provider={state.provider}
        onTriggerSync={() => state.syncMutation.mutate()}
      />
    </section>
  )
}
