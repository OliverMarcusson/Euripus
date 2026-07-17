import { ProviderSyncActivityCard } from "@/features/provider/provider-settings-cards";
import { useProviderSettingsForm } from "@/features/provider/use-provider-settings-form";

export function RestrictedProviderSyncSection() {
  const state = useProviderSettingsForm();

  return (
    <ProviderSyncActivityCard
      latestJob={state.latestJob}
      syncProgressValue={state.syncProgressValue}
      syncErrorMessage={state.syncErrorMessage}
      syncPending={state.syncMutation.isPending}
      syncBlockedByActiveJob={state.syncBlockedByActiveJob}
      provider={state.provider}
      onTriggerSync={() => state.syncMutation.mutate()}
    />
  );
}
