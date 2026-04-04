import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { useForm } from "react-hook-form";
import { saveProvider, getProvider, getSyncStatus, triggerProviderSync, validateProvider } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { formatDateTime } from "@/lib/utils";

const providerSchema = z.object({
  baseUrl: z.string().url(),
  username: z.string().min(1),
  password: z.string().min(1),
  outputFormat: z.enum(["m3u8", "ts"]),
});

type ProviderValues = z.infer<typeof providerSchema>;

export function ProviderPage() {
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

  const validationMessage = validateMutation.data?.message;
  const provider = providerQuery.data;

  return (
    <div className="flex flex-col gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Xtreme Codes Provider</CardTitle>
          <CardDescription>Store one synced Xtreme Codes provider profile per account. Credentials are encrypted on the server.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
          <form className="flex flex-col gap-4" onSubmit={form.handleSubmit((values) => saveMutation.mutate(values))}>
            <div className="flex flex-col gap-2">
              <label className="text-sm font-medium" htmlFor="baseUrl">
                Base URL
              </label>
              <Input id="baseUrl" defaultValue={provider?.baseUrl} placeholder="https://provider.example.com" {...form.register("baseUrl")} />
            </div>
            <div className="grid gap-4 md:grid-cols-2">
              <div className="flex flex-col gap-2">
                <label className="text-sm font-medium" htmlFor="providerUsername">
                  Provider username
                </label>
                <Input id="providerUsername" defaultValue={provider?.username} {...form.register("username")} />
              </div>
              <div className="flex flex-col gap-2">
                <label className="text-sm font-medium" htmlFor="providerPassword">
                  Provider password
                </label>
                <Input id="providerPassword" type="password" {...form.register("password")} />
              </div>
            </div>
            <div className="flex flex-col gap-2">
              <label className="text-sm font-medium" htmlFor="outputFormat">
                Preferred output format
              </label>
              <select
                id="outputFormat"
                className="flex h-11 rounded-lg border border-input bg-card px-3 py-2 text-sm"
                defaultValue={provider?.outputFormat ?? "m3u8"}
                {...form.register("outputFormat")}
              >
                <option value="m3u8">m3u8 (recommended)</option>
                <option value="ts">ts</option>
              </select>
            </div>
            <div className="flex flex-wrap gap-3">
              <Button
                type="button"
                variant="secondary"
                onClick={form.handleSubmit((values) => validateMutation.mutate(values))}
                disabled={validateMutation.isPending}
              >
                Validate
              </Button>
              <Button type="submit" disabled={saveMutation.isPending}>
                Save profile
              </Button>
            </div>
            {validationMessage ? <p className="text-sm text-muted-foreground">{validationMessage}</p> : null}
          </form>
          <div className="flex flex-col gap-4 rounded-2xl border border-border bg-card/60 p-5">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold">Sync status</h3>
              <Badge>{provider?.status ?? "missing"}</Badge>
            </div>
            <p className="text-sm text-muted-foreground">Last validated: {formatDateTime(provider?.lastValidatedAt ?? null)}</p>
            <p className="text-sm text-muted-foreground">Last synced: {formatDateTime(provider?.lastSyncAt ?? null)}</p>
            <p className="text-sm text-muted-foreground">Latest job: {syncQuery.data?.status ?? "idle"}</p>
            {provider?.lastSyncError ? <p className="text-sm text-destructive">{provider.lastSyncError}</p> : null}
            <Button onClick={() => syncMutation.mutate()} disabled={syncMutation.isPending || !provider}>
              Trigger full sync
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
