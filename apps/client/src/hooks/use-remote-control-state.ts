import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ReceiverDevice } from "@euripus/shared";
import {
  getRemoteControllerTarget,
  getRemoteReceivers,
} from "@/lib/api";
import { REMOTE_QUERY_STALE_TIME_MS } from "@/lib/query-cache";
import {
  useRemoteControllerStore,
  type RemoteControllerTargetSelection,
} from "@/store/remote-controller-store";

export function useRemoteControllerTargetQuery({
  enabled,
  refetchInterval = false,
}: {
  enabled: boolean;
  refetchInterval?: number | false;
}) {
  const setTargetSelection = useRemoteControllerStore(
    (state) => state.setTargetSelection,
  );
  const clearTarget = useRemoteControllerStore((state) => state.clearTarget);
  const query = useQuery({
    queryKey: ["remote", "controller", "target"],
    queryFn: getRemoteControllerTarget,
    enabled,
    refetchInterval,
    staleTime: REMOTE_QUERY_STALE_TIME_MS,
  });

  useEffect(() => {
    if (!enabled || query.data === undefined) {
      return;
    }

    if (query.data) {
      setTargetSelection(query.data);
      return;
    }

    clearTarget();
  }, [clearTarget, enabled, query.data, setTargetSelection]);

  return query;
}

export function useRemoteReceiversQuery({
  enabled,
  refetchInterval = false,
}: {
  enabled: boolean;
  refetchInterval?: number | false;
}) {
  return useQuery({
    queryKey: ["remote", "receivers"],
    queryFn: getRemoteReceivers,
    enabled,
    refetchInterval,
    staleTime: REMOTE_QUERY_STALE_TIME_MS,
  });
}

export function resolveRemoteTargetDevice(
  selection: RemoteControllerTargetSelection | null,
  device: ReceiverDevice | null | undefined,
) {
  if (!selection || !device || device.id !== selection.id) {
    return null;
  }

  return device;
}
