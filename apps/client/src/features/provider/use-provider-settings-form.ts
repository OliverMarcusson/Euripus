import { zodResolver } from "@hookform/resolvers/zod"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import type {
  ProviderProfile,
  SaveProviderPayload,
  SyncJob,
} from "@euripus/shared"
import { useEffect, useState } from "react"
import { useForm } from "react-hook-form"
import { z } from "zod"
import {
  getProvider,
  getSyncStatus,
  saveProvider,
  triggerProviderSync,
  validateProvider,
} from "@/lib/api"
import {
  STANDARD_QUERY_STALE_TIME_MS,
  SYNC_STATUS_STALE_TIME_MS,
} from "@/lib/query-cache"

export const providerSchema = z.object({
  baseUrl: z.string().url(),
  username: z.string().min(1),
  password: z.string(),
  outputFormat: z.enum(["m3u8", "ts"]),
  playbackMode: z.enum(["direct", "relay"]),
  epgSources: z.array(
    z.object({
      id: z.string().uuid().optional(),
      url: z.string().url(),
      enabled: z.boolean(),
      priority: z.number().int().nonnegative(),
    }),
  ),
})

export type ProviderFormValues = z.infer<typeof providerSchema>

export function createProviderFormValues(
  provider: ProviderProfile | null | undefined,
): ProviderFormValues {
  return {
    baseUrl: provider?.baseUrl ?? "",
    username: provider?.username ?? "",
    password: "",
    outputFormat: provider?.outputFormat ?? "m3u8",
    playbackMode: provider?.playbackMode ?? "direct",
    epgSources:
      provider?.epgSources.map((source) => ({
        id: source.id,
        url: source.url,
        enabled: source.enabled,
        priority: source.priority,
      })) ?? [],
  }
}

export function reindexEpgSources(
  items: SaveProviderPayload["epgSources"],
): SaveProviderPayload["epgSources"] {
  return items.map((item, index) => ({ ...item, priority: index }))
}

export function toSaveProviderPayload(
  values: ProviderFormValues,
): SaveProviderPayload {
  return {
    ...values,
    password: values.password.trim(),
    epgSources: reindexEpgSources(values.epgSources),
  }
}

export function getSyncProgressValue(syncJob: SyncJob) {
  if (!syncJob.totalPhases) {
    return 0
  }

  const inFlightPhaseBonus =
    syncJob.status === "running" ? 0.5 : syncJob.status === "succeeded" ? 1 : 0
  return Math.min(
    100,
    ((syncJob.completedPhases + inFlightPhaseBonus) / syncJob.totalPhases) *
      100,
  )
}

export function useProviderSettingsForm() {
  const queryClient = useQueryClient()
  const [feedbackMessage, setFeedbackMessage] = useState<string | null>(null)
  const providerQuery = useQuery({
    queryKey: ["provider"],
    queryFn: getProvider,
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
    refetchInterval: (query) => {
      const provider = query.state.data
      return provider?.status === "syncing" ? 1000 : false
    },
  })
  const syncQuery = useQuery({
    queryKey: ["sync-status"],
    queryFn: getSyncStatus,
    staleTime: SYNC_STATUS_STALE_TIME_MS,
    refetchInterval: (query) => {
      const latestJob = query.state.data
      return latestJob?.status === "queued" || latestJob?.status === "running"
        ? 1000
        : false
    },
  })
  const form = useForm<ProviderFormValues>({
    resolver: zodResolver(providerSchema),
    defaultValues: createProviderFormValues(null),
  })

  useEffect(() => {
    if (providerQuery.data === undefined || form.formState.isDirty) {
      return
    }

    form.reset(createProviderFormValues(providerQuery.data))
  }, [form, providerQuery.data])

  const validateMutation = useMutation({ mutationFn: validateProvider })
  const saveMutation = useMutation({
    mutationFn: saveProvider,
    onSuccess: async (provider) => {
      setFeedbackMessage(provider.browserPlaybackWarning ?? null)
      queryClient.setQueryData(["provider"], provider)
      form.reset(createProviderFormValues(provider))
      await queryClient.invalidateQueries({ queryKey: ["provider"] })
    },
  })
  const syncMutation = useMutation({
    mutationFn: triggerProviderSync,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["provider"] }),
        queryClient.invalidateQueries({ queryKey: ["sync-status"] }),
        queryClient.invalidateQueries({ queryKey: ["channels"] }),
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
      ])
    },
    onError: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["provider"] }),
        queryClient.invalidateQueries({ queryKey: ["sync-status"] }),
      ])
    },
  })

  const provider = providerQuery.data
  const latestJob = syncQuery.data
  const watchedEpgSources = form.watch("epgSources")
  const syncProgressValue = latestJob ? getSyncProgressValue(latestJob) : 0
  const syncErrorMessage =
    syncMutation.error instanceof Error ? syncMutation.error.message : null
  const syncBlockedByActiveJob =
    latestJob?.status === "queued" || latestJob?.status === "running"
  const displayedEpgSourceCount = form.formState.isDirty
    ? watchedEpgSources.length
    : provider?.epgSources.length ?? watchedEpgSources.length

  function ensurePasswordForAction(values: ProviderFormValues) {
    if (provider || values.password.trim().length > 0) {
      return true
    }

    form.setError("password", {
      type: "manual",
      message:
        "Enter your provider password when saving the profile for the first time.",
    })
    return false
  }

  const submitSave = form.handleSubmit((values) => {
    if (!ensurePasswordForAction(values)) {
      return
    }

    setFeedbackMessage(null)
    validateMutation.reset()
    saveMutation.mutate(toSaveProviderPayload(values))
  })

  const submitValidate = form.handleSubmit((values) => {
    if (!ensurePasswordForAction(values)) {
      return
    }

    setFeedbackMessage(null)
    validateMutation.reset()
    validateMutation.mutate(toSaveProviderPayload(values))
  })

  return {
    displayedEpgSourceCount,
    feedbackMessage:
      feedbackMessage ??
      validateMutation.data?.message ??
      provider?.browserPlaybackWarning ??
      null,
    form,
    latestJob,
    provider,
    providerQuery,
    saveMutation,
    submitSave,
    submitValidate,
    syncBlockedByActiveJob,
    syncErrorMessage,
    syncMutation,
    syncProgressValue,
    syncQuery,
    validateMutation,
  }
}
