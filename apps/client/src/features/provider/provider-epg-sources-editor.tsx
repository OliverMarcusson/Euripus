import { ArrowDown, ArrowUp, Link2, Plus, Trash2 } from "lucide-react"
import type { ProviderProfile } from "@euripus/shared"
import { Controller, useFieldArray, type UseFormReturn } from "react-hook-form"
import { Button } from "@/components/ui/button"
import { Field, FieldError, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  type ProviderFormValues,
  reindexEpgSources,
} from "@/features/provider/use-provider-settings-form"
import { formatRelativeTime } from "@/lib/utils"

type ProviderEpgSourcesEditorProps = {
  form: UseFormReturn<ProviderFormValues>
  provider: ProviderProfile | null | undefined
}

export function ProviderEpgSourcesEditor({
  form,
  provider,
}: ProviderEpgSourcesEditorProps) {
  const epgSourceFields = useFieldArray({
    control: form.control,
    name: "epgSources",
    keyName: "fieldId",
  })

  function reorderEpgSources(fromIndex: number, toIndex: number) {
    const current = [...form.getValues("epgSources")]
    const [moved] = current.splice(fromIndex, 1)

    if (!moved) {
      return
    }

    current.splice(toIndex, 0, moved)
    form.setValue("epgSources", reindexEpgSources(current), {
      shouldDirty: true,
    })
  }

  return (
    <div className="py-2 sm:rounded-[1.5rem] sm:border sm:border-border/40 sm:bg-black/10 sm:p-5 sm:shadow-inner">
      <div className="mb-5 flex items-center justify-between gap-3">
        <div className="space-y-1">
          <FieldLabel>External EPG sources</FieldLabel>
        </div>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          onClick={() =>
            epgSourceFields.append({
              url: "https://",
              enabled: true,
              priority: epgSourceFields.fields.length,
            })}
        >
          <Plus data-icon="inline-start" />
          Add source
        </Button>
      </div>

      <div className="flex flex-col gap-4">
        {epgSourceFields.fields.length ? (
          epgSourceFields.fields.map((field, index) => {
            const sourceHealth = provider?.epgSources.find(
              (source) => source.id === field.id,
            )

            return (
              <div
                key={field.fieldId}
                className="border-t border-border/60 py-4 first:border-t-0 first:pt-0 shadow-sm backdrop-blur-md transition-all hover:bg-secondary/40 sm:rounded-2xl sm:border sm:border-border/30 sm:bg-background/40 sm:p-5"
              >
                <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    <Link2 className="size-4 text-muted-foreground" />
                    Source {index + 1}
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      type="button"
                      size="icon"
                      variant="ghost"
                      onClick={() => reorderEpgSources(index, index - 1)}
                      disabled={index === 0}
                    >
                      <ArrowUp />
                    </Button>
                    <Button
                      type="button"
                      size="icon"
                      variant="ghost"
                      onClick={() => reorderEpgSources(index, index + 1)}
                      disabled={index === epgSourceFields.fields.length - 1}
                    >
                      <ArrowDown />
                    </Button>
                    <Button
                      type="button"
                      size="icon"
                      variant="ghost"
                      onClick={() => epgSourceFields.remove(index)}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                </div>

                <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_150px]">
                  <Field
                    data-invalid={
                      form.formState.errors.epgSources?.[index]?.url
                        ? true
                        : undefined
                    }
                  >
                    <FieldLabel htmlFor={`epg-source-url-${index}`}>
                      Feed URL
                    </FieldLabel>
                    <Input
                      id={`epg-source-url-${index}`}
                      placeholder="https://provider.example.com/guide.xml.gz"
                      aria-invalid={
                        form.formState.errors.epgSources?.[index]?.url
                          ? true
                          : undefined
                      }
                      {...form.register(`epgSources.${index}.url`)}
                    />
                    {field.id ? (
                      <input
                        type="hidden"
                        {...form.register(`epgSources.${index}.id`)}
                      />
                    ) : null}
                    <input
                      type="hidden"
                      {...form.register(`epgSources.${index}.priority`, {
                        valueAsNumber: true,
                      })}
                    />
                    <FieldError
                      errors={[form.formState.errors.epgSources?.[index]?.url]}
                    />
                  </Field>

                  <Field>
                    <FieldLabel htmlFor={`epg-source-enabled-${index}`}>
                      Status
                    </FieldLabel>
                    <Controller
                      control={form.control}
                      name={`epgSources.${index}.enabled`}
                      render={({ field: controllerField }) => (
                        <Select
                          value={controllerField.value ? "enabled" : "disabled"}
                          onValueChange={(value) =>
                            controllerField.onChange(value === "enabled")}
                        >
                          <SelectTrigger id={`epg-source-enabled-${index}`}>
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectGroup>
                              <SelectItem value="enabled">Enabled</SelectItem>
                              <SelectItem value="disabled">Disabled</SelectItem>
                            </SelectGroup>
                          </SelectContent>
                        </Select>
                      )}
                    />
                  </Field>
                </div>

                {sourceHealth ? (
                  <div className="mt-3 text-sm text-muted-foreground sm:rounded-xl sm:border sm:border-border/70 sm:bg-muted/40 sm:p-3">
                    <div>
                      Last sync{" "}
                      {sourceHealth.lastSyncAt
                        ? formatRelativeTime(sourceHealth.lastSyncAt)
                        : "not run yet"}
                    </div>
                    <div>
                      Parsed {sourceHealth.lastProgramCount ?? 0} programs,
                      matched {sourceHealth.lastMatchedCount ?? 0}
                    </div>
                    {sourceHealth.lastSyncError ? (
                      <div className="text-destructive">
                        {sourceHealth.lastSyncError}
                      </div>
                    ) : null}
                  </div>
                ) : null}
              </div>
            )
          })
        ) : (
          <div className="border-t border-dashed border-border/70 py-4 text-sm text-muted-foreground sm:rounded-2xl sm:border sm:bg-background/40 sm:p-4">
            No external EPG sources
          </div>
        )}
      </div>
    </div>
  )
}
