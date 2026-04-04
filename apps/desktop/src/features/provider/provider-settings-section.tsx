import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, RefreshCcw, ServerCog } from "lucide-react";
import { useEffect } from "react";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { getProvider, getSyncStatus, saveProvider, triggerProviderSync, validateProvider } from "@/lib/api";
import { formatDateTime, formatRelativeTime } from "@/lib/utils";

const providerSchema = z.object({
  baseUrl: z.string().url(),
  username: z.string().min(1),
  password: z.string().min(1),
  outputFormat: z.enum(["m3u8", "ts"]),
});

type ProviderValues = z.infer<typeof providerSchema>;

export function ProviderSettingsSection() {
  const queryClient = useQueryClient();
  const providerQuery = useQuery({ queryKey: ["provider"], queryFn: getProvider });
  const syncQuery = useQuery({ queryKey: ["sync-status"], queryFn: getSyncStatus });
  const form = useForm<ProviderValues>({
    resolver: zodResolver(providerSchema),
    defaultValues: {
      baseUrl: "",
      username: "",
      password: "",
      outputFormat: "m3u8",
    },
  });

  useEffect(() => {
    if (!providerQuery.data) {
      return;
    }

    form.reset({
      baseUrl: providerQuery.data.baseUrl ?? "",
      username: providerQuery.data.username ?? "",
      password: "",
      outputFormat: providerQuery.data.outputFormat ?? "m3u8",
    });
  }, [providerQuery.data, form]);

  const validateMutation = useMutation({ mutationFn: validateProvider });
  const saveMutation = useMutation({
    mutationFn: saveProvider,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["provider"] });
    },
  });
  const syncMutation = useMutation({
    mutationFn: triggerProviderSync,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["sync-status"] }),
        queryClient.invalidateQueries({ queryKey: ["channels"] }),
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
      ]);
    },
  });

  const provider = providerQuery.data;
  const latestJob = syncQuery.data;

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_360px]">
      <Card className="overflow-hidden border-border/80 bg-gradient-to-br from-card via-card to-primary/5">
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <CardTitle>Provider</CardTitle>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={provider?.status === "valid" ? "accent" : provider?.status === "error" ? "destructive" : "outline"}>
              {provider?.status ?? "missing"}
            </Badge>
            <Badge variant="outline">{latestJob?.status ?? "idle"}</Badge>
          </div>
        </CardHeader>
        <CardContent>
          <form className="flex flex-col gap-6" onSubmit={form.handleSubmit((values) => saveMutation.mutate(values))}>
            <FieldGroup>
              <Field data-invalid={form.formState.errors.baseUrl ? true : undefined}>
                <FieldLabel htmlFor="baseUrl">Base URL</FieldLabel>
                <Input
                  id="baseUrl"
                  placeholder="https://provider.example.com"
                  aria-invalid={form.formState.errors.baseUrl ? true : undefined}
                  {...form.register("baseUrl")}
                />
                <FieldError errors={[form.formState.errors.baseUrl]} />
              </Field>

              <div className="grid gap-5 md:grid-cols-2">
                <Field data-invalid={form.formState.errors.username ? true : undefined}>
                  <FieldLabel htmlFor="providerUsername">Provider username</FieldLabel>
                  <Input
                    id="providerUsername"
                    aria-invalid={form.formState.errors.username ? true : undefined}
                    {...form.register("username")}
                  />
                  <FieldError errors={[form.formState.errors.username]} />
                </Field>

                <Field data-invalid={form.formState.errors.password ? true : undefined}>
                  <FieldLabel htmlFor="providerPassword">Provider password</FieldLabel>
                  <Input
                    id="providerPassword"
                    type="password"
                    aria-invalid={form.formState.errors.password ? true : undefined}
                    {...form.register("password")}
                  />
                  <FieldError errors={[form.formState.errors.password]} />
                </Field>
              </div>

              <Field data-invalid={form.formState.errors.outputFormat ? true : undefined}>
                <FieldLabel htmlFor="outputFormat">Preferred output format</FieldLabel>
                <Controller
                  control={form.control}
                  name="outputFormat"
                  render={({ field }) => (
                    <Select value={field.value} onValueChange={field.onChange}>
                      <SelectTrigger id="outputFormat" aria-invalid={form.formState.errors.outputFormat ? true : undefined}>
                        <SelectValue placeholder="Select format" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          <SelectItem value="m3u8">M3U8</SelectItem>
                          <SelectItem value="ts">TS</SelectItem>
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  )}
                />
                <FieldError errors={[form.formState.errors.outputFormat]} />
              </Field>
            </FieldGroup>

            <div className="flex flex-wrap items-center gap-3">
              <Button
                type="button"
                variant="secondary"
                onClick={form.handleSubmit((values) => validateMutation.mutate(values))}
                disabled={validateMutation.isPending}
              >
                <CheckCircle2 data-icon="inline-start" />
                {validateMutation.isPending ? "Validating..." : "Validate"}
              </Button>
              <Button type="submit" disabled={saveMutation.isPending}>
                <ServerCog data-icon="inline-start" />
                {saveMutation.isPending ? "Saving..." : "Save profile"}
              </Button>
            </div>

            {validateMutation.data?.message ? (
              <div className="rounded-2xl border border-border/70 bg-muted/50 p-4 text-sm">
                {validateMutation.data.message}
              </div>
            ) : null}
          </form>
        </CardContent>
      </Card>

      <div className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Profile health</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <StatusRow
              label="Provider status"
              value={provider?.status ?? "missing"}
              detail={
                provider?.lastValidatedAt
                  ? `Last validated ${formatRelativeTime(provider.lastValidatedAt)}`
                  : "Not validated"
              }
            />
            <Separator />
            <StatusRow
              label="Last sync"
              value={provider?.lastSyncAt ? formatRelativeTime(provider.lastSyncAt) : "Never"}
              detail={formatDateTime(provider?.lastSyncAt ?? null)}
            />
            <Separator />
            <StatusRow
              label="Output format"
              value={provider?.outputFormat?.toUpperCase() ?? form.watch("outputFormat").toUpperCase()}
              detail="Used for future playback."
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

        <Card>
          <CardHeader>
            <CardTitle>Sync activity</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <StatusRow
              label="Latest job"
              value={latestJob?.status ?? "idle"}
              detail={latestJob?.createdAt ? `Created ${formatDateTime(latestJob.createdAt)}` : "No job recorded"}
            />
            <StatusRow
              label="Job window"
              value={latestJob?.jobType ?? "full"}
              detail={
                latestJob?.startedAt
                  ? `${formatDateTime(latestJob.startedAt)} to ${formatDateTime(latestJob.finishedAt ?? latestJob.startedAt)}`
                  : "Run a sync to populate timing details"
              }
            />
            {latestJob?.errorMessage ? (
              <div className="rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                {latestJob.errorMessage}
              </div>
            ) : null}
            <Button onClick={() => syncMutation.mutate()} disabled={syncMutation.isPending || !provider}>
              <RefreshCcw data-icon="inline-start" />
              {syncMutation.isPending ? "Syncing..." : "Trigger full sync"}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function StatusRow({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-base font-semibold capitalize">{value}</span>
      <span className="text-sm text-muted-foreground">{detail}</span>
    </div>
  );
}
