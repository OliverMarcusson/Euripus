import { zodResolver } from "@hookform/resolvers/zod"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import type {
  ProviderProfile,
  SaveProviderPayload,
  SyncJob,
} from "@euripus/shared"
import { useEffect, useRef, useState } from "react"
import { useForm, useWatch } from "react-hook-form"
import { z } from "zod"
import {
  deleteProvider,
  getProviders,
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

function sortProviders(providers: ProviderProfile[]) {
  return [...providers].sort((left, right) => {
    if (left.updatedAt === right.updatedAt) {
      return right.createdAt.localeCompare(left.createdAt)
    }
    return right.updatedAt.localeCompare(left.updatedAt)
  })
}

export function reindexEpgSources(
  items: SaveProviderPayload["epgSources"],
): SaveProviderPayload["epgSources"] {
  return items.map((item, index) => ({ ...item, priority: index }))
}

export function toSaveProviderPayload(
  values: ProviderFormValues,
  providerId?: string,
): SaveProviderPayload {
  return {
    id: providerId,
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
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null)
  const [isCreatingProvider, setIsCreatingProvider] = useState(false)
  const lastResetKeyRef = useRef<string | null>(null)

  const providersQuery = useQuery({
    queryKey: ["providers"],
    queryFn: getProviders,
    staleTime: STANDARD_QUERY_STALE_TIME_MS,
    refetchInterval: (query) => {
      const providers = query.state.data
      return providers?.some((provider) => provider.status === "syncing")
        ? 1000
        : false
    },
  })

  const providers = providersQuery.data ?? []
  const selectedProvider = isCreatingProvider
    ? null
    : providers.find((provider) => provider.id === selectedProviderId) ??
      providers[0] ??
      null

  const syncQuery = useQuery({
    queryKey: ["sync-status", selectedProvider?.id],
    queryFn: () => getSyncStatus(selectedProvider!.id),
    enabled: selectedProvider !== null,
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
    if (providersQuery.data === undefined) {
      return
    }

    if (providers.length === 0) {
      const nextKey = "new"
      setSelectedProviderId(null)
      if (lastResetKeyRef.current !== nextKey) {
        form.reset(createProviderFormValues(null))
        lastResetKeyRef.current = nextKey
      }
      return
    }

    if (isCreatingProvider) {
      const nextKey = "new"
      if (lastResetKeyRef.current !== nextKey) {
        form.reset(createProviderFormValues(null))
        lastResetKeyRef.current = nextKey
      }
      return
    }

    const nextSelectedProvider =
      providers.find((provider) => provider.id === selectedProviderId) ?? providers[0]

    if (!nextSelectedProvider) {
      return
    }

    if (nextSelectedProvider.id !== selectedProviderId) {
      setSelectedProviderId(nextSelectedProvider.id)
      return
    }

    if (lastResetKeyRef.current !== nextSelectedProvider.id) {
      form.reset(createProviderFormValues(nextSelectedProvider))
      lastResetKeyRef.current = nextSelectedProvider.id
    }
  }, [
    form,
    isCreatingProvider,
    providers,
    providersQuery.data,
    selectedProviderId,
  ])

  const validateMutation = useMutation({ mutationFn: validateProvider })
  const saveMutation = useMutation({
    mutationFn: saveProvider,
    onSuccess: async (provider) => {
      setFeedbackMessage(provider.browserPlaybackWarning ?? null)
      setIsCreatingProvider(false)
      setSelectedProviderId(provider.id)
      lastResetKeyRef.current = provider.id
      queryClient.setQueryData<ProviderProfile[]>(["providers"], (current) => {
        const next = current ? [...current] : []
        const existingIndex = next.findIndex((item) => item.id === provider.id)
        if (existingIndex >= 0) {
          next[existingIndex] = provider
        } else {
          next.push(provider)
        }
        return sortProviders(next)
      })
      form.reset(createProviderFormValues(provider))
      await queryClient.invalidateQueries({ queryKey: ["providers"] })
      await queryClient.invalidateQueries({
        queryKey: ["sync-status", provider.id],
      })
    },
  })
  const deleteMutation = useMutation({
    mutationFn: deleteProvider,
    onSuccess: async (_, deletedProviderId) => {
      setFeedbackMessage(null)
      validateMutation.reset()
      saveMutation.reset()
      syncMutation.reset()
      queryClient.setQueryData<ProviderProfile[]>(["providers"], (current) => {
        const next = (current ?? []).filter(
          (provider) => provider.id !== deletedProviderId,
        )
        return sortProviders(next)
      })
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["providers"] }),
        queryClient.invalidateQueries({ queryKey: ["channels"] }),
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
        queryClient.invalidateQueries({ queryKey: ["sync-status"] }),
      ])
    },
  })
  const syncMutation = useMutation({
    mutationFn: async () => {
      if (!selectedProvider) {
        throw new Error("Select a provider before triggering sync.")
      }
      return triggerProviderSync(selectedProvider.id)
    },
    onSuccess: async () => {
      if (!selectedProvider) {
        return
      }

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["providers"] }),
        queryClient.invalidateQueries({
          queryKey: ["sync-status", selectedProvider.id],
        }),
        queryClient.invalidateQueries({ queryKey: ["channels"] }),
        queryClient.invalidateQueries({ queryKey: ["guide"] }),
      ])
    },
    onError: async () => {
      if (!selectedProvider) {
        return
      }

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["providers"] }),
        queryClient.invalidateQueries({
          queryKey: ["sync-status", selectedProvider.id],
        }),
      ])
    },
  })

  const provider = selectedProvider
  const latestJob = syncQuery.data
  const watchedEpgSources = useWatch({
    control: form.control,
    name: "epgSources",
  })
  const syncProgressValue = latestJob ? getSyncProgressValue(latestJob) : 0
  const syncErrorMessage =
    syncMutation.error instanceof Error ? syncMutation.error.message : null
  const syncBlockedByActiveJob =
    latestJob?.status === "queued" || latestJob?.status === "running"
  const displayedEpgSourceCount = form.formState.isDirty
    ? watchedEpgSources.length
    : provider?.epgSources.length ?? watchedEpgSources.length

  function resetTransientState() {
    setFeedbackMessage(null)
    validateMutation.reset()
    saveMutation.reset()
    deleteMutation.reset()
    syncMutation.reset()
  }

  function selectProvider(providerId: string) {
    setIsCreatingProvider(false)
    setSelectedProviderId(providerId)
    resetTransientState()
  }

  function startCreatingProvider() {
    setIsCreatingProvider(true)
    setSelectedProviderId(null)
    resetTransientState()
  }

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
    saveMutation.mutate(toSaveProviderPayload(values, provider?.id))
  })

  const submitValidate = form.handleSubmit((values) => {
    if (!ensurePasswordForAction(values)) {
      return
    }

    setFeedbackMessage(null)
    validateMutation.reset()
    validateMutation.mutate(toSaveProviderPayload(values, provider?.id))
  })

  function submitDelete() {
    if (!provider) {
      return
    }

    if (
      typeof window !== "undefined" &&
      !window.confirm(`Delete provider ${provider.username}? This removes its synced channels, guide data, and sync history.`)
    ) {
      return
    }

    deleteMutation.mutate(provider.id)
  }

  return {
    displayedEpgSourceCount,
    feedbackMessage:
      feedbackMessage ??
      validateMutation.data?.message ??
      provider?.browserPlaybackWarning ??
      null,
    form,
    deleteMutation,
    isCreatingProvider,
    latestJob,
    provider,
    providers,
    providersQuery,
    saveMutation,
    selectedProviderId: provider?.id ?? null,
    selectProvider,
    startCreatingProvider,
    submitDelete,
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
