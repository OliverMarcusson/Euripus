import type { ProviderProfile, SyncJob } from "@euripus/shared"
import { CheckCircle2, ServerCog } from "lucide-react"
import type { FormEventHandler } from "react"
import { Controller, type UseFormReturn } from "react-hook-form"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { ProviderEpgSourcesEditor } from "@/features/provider/provider-epg-sources-editor"
import type { ProviderFormValues } from "@/features/provider/use-provider-settings-form"

type ProviderSettingsFormProps = {
  form: UseFormReturn<ProviderFormValues>
  provider: ProviderProfile | null | undefined
  latestJob: SyncJob | null | undefined
  savePending: boolean
  validatePending: boolean
  validationMessage?: string
  onSubmit: FormEventHandler<HTMLFormElement>
  onValidate: () => void
}

export function ProviderSettingsForm({
  form,
  provider,
  latestJob,
  savePending,
  validatePending,
  validationMessage,
  onSubmit,
  onValidate,
}: ProviderSettingsFormProps) {
  return (
    <Card className="self-start overflow-hidden rounded-none border-0 bg-transparent shadow-none sm:rounded-3xl sm:border sm:border-border/50 sm:bg-card/40 sm:backdrop-blur-xl sm:shadow-2xl">
      <CardHeader className="flex flex-row items-start justify-between gap-4 px-0 pt-0 pb-4 sm:p-6 sm:pb-0">
        <CardTitle className="text-xl font-medium tracking-tight">
          Provider
        </CardTitle>
        <div className="flex flex-wrap items-center gap-2">
          <Badge
            variant={
              provider?.status === "valid"
                ? "accent"
                : provider?.status === "error"
                  ? "destructive"
                  : "outline"
            }
          >
            {provider?.status ?? "missing"}
          </Badge>
          <Badge variant="outline">{latestJob?.status ?? "idle"}</Badge>
        </div>
      </CardHeader>
      <CardContent className="px-0 pb-0 sm:p-6">
        <form className="flex flex-col gap-6" onSubmit={onSubmit}>
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
              <Field
                data-invalid={form.formState.errors.username ? true : undefined}
              >
                <FieldLabel htmlFor="providerUsername">
                  Provider username
                </FieldLabel>
                <Input
                  id="providerUsername"
                  aria-invalid={form.formState.errors.username ? true : undefined}
                  {...form.register("username")}
                />
                <FieldError errors={[form.formState.errors.username]} />
              </Field>

              <Field
                data-invalid={form.formState.errors.password ? true : undefined}
              >
                <FieldLabel htmlFor="providerPassword">
                  Provider password
                </FieldLabel>
                <Input
                  id="providerPassword"
                  type="password"
                  aria-invalid={form.formState.errors.password ? true : undefined}
                  {...form.register("password")}
                />
                <FieldError errors={[form.formState.errors.password]} />
              </Field>
            </div>

            <Field
              data-invalid={form.formState.errors.outputFormat ? true : undefined}
            >
              <FieldLabel htmlFor="outputFormat">
                Receiver/native output format
              </FieldLabel>
              <FieldDescription>
                Browser playback always uses HLS. This setting is reserved for
                receiver/native compatibility when a provider prefers TS.
              </FieldDescription>
              <Controller
                control={form.control}
                name="outputFormat"
                render={({ field }) => (
                  <Select value={field.value} onValueChange={field.onChange}>
                    <SelectTrigger
                      id="outputFormat"
                      aria-invalid={
                        form.formState.errors.outputFormat ? true : undefined
                      }
                    >
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

            <Field
              data-invalid={form.formState.errors.playbackMode ? true : undefined}
            >
              <FieldLabel htmlFor="playbackMode">Playback routing</FieldLabel>
              <Controller
                control={form.control}
                name="playbackMode"
                render={({ field }) => (
                  <Select value={field.value} onValueChange={field.onChange}>
                    <SelectTrigger
                      id="playbackMode"
                      aria-invalid={
                        form.formState.errors.playbackMode ? true : undefined
                      }
                    >
                      <SelectValue placeholder="Select routing mode" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="direct">
                          Direct provider connection
                        </SelectItem>
                        <SelectItem value="relay">
                          Relay through Euripus
                        </SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                )}
              />
              <FieldError errors={[form.formState.errors.playbackMode]} />
            </Field>

            <ProviderEpgSourcesEditor form={form} provider={provider} />
          </FieldGroup>

          <div className="flex flex-wrap items-center gap-3">
            <Button
              type="button"
              variant="secondary"
              onClick={onValidate}
              disabled={validatePending}
            >
              <CheckCircle2 data-icon="inline-start" />
              {validatePending ? "Validating..." : "Validate"}
            </Button>
            <Button type="submit" disabled={savePending}>
              <ServerCog data-icon="inline-start" />
              {savePending ? "Saving..." : "Save profile"}
            </Button>
          </div>

          {validationMessage ? (
            <div className="rounded-2xl border border-border/70 bg-muted/50 p-4 text-sm">
              {validationMessage}
            </div>
          ) : null}
        </form>
      </CardContent>
    </Card>
  )
}
