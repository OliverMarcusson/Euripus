import { ServerNetworkStatusCard } from "@/components/server/server-network-status-card"
import { Separator } from "@/components/ui/separator"
import {
  ProviderHealthCard,
  ProviderSyncActivityCard,
} from "@/features/provider/provider-settings-cards"
import { ProviderSettingsForm } from "@/features/provider/provider-settings-form"
import { useProviderSettingsForm } from "@/features/provider/use-provider-settings-form"

export function ProviderSettingsSection() {
  const state = useProviderSettingsForm()
  const outputFormat =
    state.provider?.outputFormat?.toUpperCase() ??
    state.form.watch("outputFormat").toUpperCase()
  const playbackMode =
    state.provider?.playbackMode ?? state.form.watch("playbackMode")

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_360px]">
      <ProviderSettingsForm
        form={state.form}
        provider={state.provider}
        latestJob={state.latestJob}
        savePending={state.saveMutation.isPending}
        validatePending={state.validateMutation.isPending}
        validationMessage={state.validateMutation.data?.message}
        onSubmit={state.submitSave}
        onValidate={state.submitValidate}
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
