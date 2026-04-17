import { ArrowDown, ArrowUp } from "lucide-react";
import type { Program } from "@euripus/shared";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  formatTimeRange,
  getProgramPlaybackState,
  getTimeProgress,
  type ProgramPlaybackState,
} from "@/lib/utils";

export function FavoriteProgramDetails({
  program,
}: {
  program: Program;
}) {
  const playbackState = getProgramPlaybackState(program);
  const isLive = playbackState === "live";

  return (
    <div className="flex min-w-0 flex-col gap-2">
      <div className="flex flex-wrap items-start gap-x-2 gap-y-1">
        <p className="min-w-0 break-words text-sm font-semibold leading-6">
          {program.title}
        </p>
        <ProgramStateBadge state={playbackState} />
      </div>
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        <span className="font-medium text-foreground/80">
          {formatTimeRange(program.startAt, program.endAt)}
        </span>
        {program.canCatchup ? (
          <Badge variant="outline" className="h-5 px-1.5 py-0 text-[10px] font-medium uppercase tracking-widest opacity-80">
            Catch-up window
          </Badge>
        ) : null}
      </div>
      {program.description ? (
        <p className="line-clamp-2 max-w-4xl text-sm leading-relaxed text-muted-foreground">
          {program.description}
        </p>
      ) : null}
      {isLive ? (
        <Progress
          value={getTimeProgress(program.startAt, program.endAt)}
          className="mt-2 h-1.5 bg-border/50"
        />
      ) : null}
    </div>
  );
}

function ProgramStateBadge({ state }: { state: ProgramPlaybackState }) {
  if (state === "live") {
    return <Badge variant="accent">Live now</Badge>;
  }

  if (state === "catchup") {
    return <Badge variant="live">Catch-up</Badge>;
  }

  if (state === "upcoming") {
    return <Badge variant="outline">Upcoming</Badge>;
  }

  return <Badge variant="outline">Info only</Badge>;
}

export function MoveButtons({
  onMoveUp,
  onMoveDown,
  canMoveUp,
  canMoveDown,
  disabled,
}: {
  onMoveUp: () => void;
  onMoveDown: () => void;
  canMoveUp: boolean;
  canMoveDown: boolean;
  disabled: boolean;
}) {
  return (
    <>
      <Button
        variant="outline"
        size="icon"
        className="shrink-0"
        onClick={onMoveUp}
        disabled={disabled || !canMoveUp}
        aria-label="Move up"
      >
        <ArrowUp className="size-4" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        className="shrink-0"
        onClick={onMoveDown}
        disabled={disabled || !canMoveDown}
        aria-label="Move down"
      >
        <ArrowDown className="size-4" />
      </Button>
    </>
  );
}

export function moveEntry<T>(entries: T[], index: number, direction: -1 | 1) {
  const nextIndex = index + direction;
  if (nextIndex < 0 || nextIndex >= entries.length) {
    return entries;
  }

  const nextEntries = [...entries];
  const [entry] = nextEntries.splice(index, 1);
  nextEntries.splice(nextIndex, 0, entry);
  return nextEntries;
}
