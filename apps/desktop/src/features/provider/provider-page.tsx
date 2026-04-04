import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { z } from "zod";
import { Controller, useForm } from "react-hook-form";
import { saveProvider, getProvider, getSyncStatus, triggerProviderSync, validateProvider } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
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

  useEffect(() => {
    if (providerQuery.data) {
      form.reset({
        baseUrl: providerQuery.data.baseUrl ?? "",
        username: providerQuery.data.username ?? "",
        password: "",
        outputFormat: providerQuery.data.outputFormat ?? "m3u8",
      });
    }
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
              <Label htmlFor="baseUrl">Base URL</Label>
              <Input id="baseUrl" placeholder="https://provider.example.com" {...form.register("baseUrl")} />
            </div>
            <div className="grid gap-4 md:grid-cols-2">
              <div className="flex flex-col gap-2">
                <Label htmlFor="providerUsername">Provider username</Label>
                <Input id="providerUsername" {...form.register("username")} />
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="providerPassword">Provider password</Label>
                <Input id="providerPassword" type="password" {...form.register("password")} />
              </div>
            </div>
            <div className="flex flex-col gap-2">
              <Label>Preferred output format</Label>
              <Controller
                control={form.control}
                name="outputFormat"
                render={({ field }) => (
                  <Select value={field.value} onValueChange={field.onChange}>
                    <SelectTrigger>
                      <SelectValue placeholder="Select format" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="m3u8">m3u8 (recommended)</SelectItem>
                      <SelectItem value="ts">ts</SelectItem>
                    </SelectContent>
                  </Select>
                )}
              />
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
