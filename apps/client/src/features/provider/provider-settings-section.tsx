import type { ProviderProfile } from "@euripus/shared";
import { Plus } from "lucide-react";
import { ServerNetworkStatusCard } from "@/components/server/server-network-status-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  ProviderHealthCard,
  ProviderSyncActivityCard,
} from "@/features/provider/provider-settings-cards";
import { ProviderSettingsForm } from "@/features/provider/provider-settings-form";
import { useProviderSettingsForm } from "@/features/provider/use-provider-settings-form";

function formatProviderLabel(provider: ProviderProfile) {
  try {
    const url = new URL(provider.baseUrl);
    return `${provider.username} · ${url.host}`;
  } catch {
    return `${provider.username} · ${provider.baseUrl}`;
  }
}

export function ProviderSettingsSection() {
  const state = useProviderSettingsForm();
  const outputFormat =
    state.provider?.outputFormat?.toUpperCase() ??
    state.form.watch("outputFormat").toUpperCase();
  const playbackMode =
    state.provider?.playbackMode ?? state.form.watch("playbackMode");

  return (
    <div className="flex flex-col">
      <Card className="rounded-none border-0 border-t border-border/60 bg-transparent py-8 shadow-none sm:py-10">
        <CardHeader className="px-0 pb-6 pt-0">
          <div className="flex items-center justify-between gap-3">
            <CardTitle className="text-xl font-medium tracking-tight">
              Providers
            </CardTitle>
            <Badge variant="outline">{state.providers.length}</Badge>
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 px-0 pb-0">
          <Button
            type="button"
            variant="outline"
            className="w-fit"
            onClick={state.startCreatingProvider}
          >
            <Plus data-icon="inline-start" />
            Add provider
          </Button>

          {state.providers.length ? (
            <div className="grid gap-2 sm:grid-cols-2">
              {state.providers.map((provider) => {
                const isSelected =
                  !state.isCreatingProvider &&
                  state.selectedProviderId === provider.id;

                return (
                  <button
                    key={provider.id}
                    type="button"
                    onClick={() => state.selectProvider(provider.id)}
                    className="flex flex-col items-start gap-1 rounded-lg border border-border/60 px-4 py-3 text-left transition-colors hover:bg-muted/50 data-[state=active]:border-primary/50 data-[state=active]:bg-muted"
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
                );
              })}
            </div>
          ) : (
            <div className="rounded-2xl border border-dashed border-border/70 bg-muted/30 p-4 text-sm text-muted-foreground">
              No providers saved yet. Add your first IPTV provider to start
              syncing.
            </div>
          )}
        </CardContent>
      </Card>

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

      <div className="flex flex-col">
        <ProviderHealthCard
          provider={state.provider}
          outputFormat={outputFormat}
          playbackMode={playbackMode}
          displayedEpgSourceCount={state.displayedEpgSourceCount}
        />

        <ServerNetworkStatusCard className="rounded-none border-0 border-t border-border/60 bg-transparent py-8 shadow-none sm:rounded-none sm:border-x-0 sm:border-b-0 sm:bg-transparent sm:py-10 sm:shadow-none" />

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
  );
}
