import { Badge } from "@/components/ui/badge";
import {
  formatHistoryEventShort,
  formatHistorySeen,
  getPortSummaries,
  getPortSummary,
  getPortTimeline,
  groupTimelineByDay,
  type PortHistoryEvent,
  type PortSummary,
} from "@/lib/port-history";
import { cn } from "@/lib/utils";

const DAY_GROUP_LABELS = {
  today: "Today",
  yesterday: "Yesterday",
  earlier: "Earlier",
} as const;

function EventKindBadge({ kind }: { kind: PortHistoryEvent["kind"] }) {
  return (
    <Badge
      variant="outline"
      className={cn(
        "shrink-0 text-[10px] uppercase",
        kind === "occupied" &&
          "border-emerald-500/40 text-emerald-600 dark:text-emerald-400",
        kind === "freed" && "border-sky-500/40 text-sky-600 dark:text-sky-400",
      )}
    >
      {kind}
    </Badge>
  );
}

function PortSummaryBar({ summary }: { summary: PortSummary }) {
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
      <span>
        First seen{" "}
        <span className="font-medium text-foreground">
          {formatHistorySeen(summary.firstSeen)}
        </span>
      </span>
      <span>
        Last seen{" "}
        <span className="font-medium text-foreground">
          {formatHistorySeen(summary.lastSeen)}
        </span>
      </span>
      <span>
        {summary.eventCount} event{summary.eventCount === 1 ? "" : "s"}
      </span>
    </div>
  );
}

function TimelineDayGroup({
  label,
  events,
}: {
  label: string;
  events: PortHistoryEvent[];
}) {
  if (events.length === 0) {
    return null;
  }

  return (
    <div className="space-y-1.5">
      <p className="text-[10px] font-semibold tracking-wide text-muted-foreground uppercase">
        {label}
      </p>
      <ul className="space-y-1">
        {events.map((event, index) => (
          <li
            key={`${event.timestamp}-${event.kind}-${event.pid}-${index}`}
            className="flex items-start gap-2 rounded-md border bg-background px-2 py-1.5"
          >
            <EventKindBadge kind={event.kind} />
            <span className="min-w-0 flex-1 break-words text-xs leading-relaxed text-muted-foreground">
              {formatHistoryEventShort(event)}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

interface PortHistoryTimelineProps {
  port: number;
  compact?: boolean;
  className?: string;
}

export function PortHistoryTimeline({
  port,
  compact = false,
  className,
}: PortHistoryTimelineProps) {
  const summary = getPortSummary(port);
  const grouped = groupTimelineByDay(getPortTimeline(port));

  if (!summary) {
    return (
      <p className={cn("text-xs text-muted-foreground", className)}>
        No history recorded for port {port} yet.
      </p>
    );
  }

  if (compact) {
    return (
      <div className={cn("space-y-1", className)}>
        <PortSummaryBar summary={summary} />
        <p className="text-xs text-muted-foreground">
          Last: {summary.lastKind} by {summary.lastProcessName}
        </p>
      </div>
    );
  }

  return (
    <div className={cn("space-y-3", className)}>
      <PortSummaryBar summary={summary} />
      <div className="max-h-48 space-y-3 overflow-y-auto pr-0.5">
        {(Object.keys(DAY_GROUP_LABELS) as Array<keyof typeof DAY_GROUP_LABELS>).map(
          (group) => (
            <TimelineDayGroup
              key={group}
              label={DAY_GROUP_LABELS[group]}
              events={grouped[group]}
            />
          ),
        )}
      </div>
    </div>
  );
}

interface PortHistoryListProps {
  onSelectPort?: (port: number) => void;
  selectedPort?: number | null;
  className?: string;
}

export function PortHistoryList({
  onSelectPort,
  selectedPort = null,
  className,
}: PortHistoryListProps) {
  const summaries = getPortSummaries();

  if (summaries.length === 0) {
    return (
      <p className={cn("text-xs text-muted-foreground", className)}>
        No history yet. Events appear when ports are occupied or freed during scans.
      </p>
    );
  }

  return (
    <div className={cn("space-y-1.5", className)}>
      {summaries.map((summary) => {
        const selected = selectedPort === summary.port;

        return (
          <div key={summary.port} className="overflow-hidden rounded-md border">
            <button
              type="button"
              className={cn(
                "flex w-full items-start justify-between gap-3 px-2.5 py-2 text-left transition-colors hover:bg-muted/40",
                selected && "bg-muted/30",
              )}
              onClick={() => onSelectPort?.(summary.port)}
            >
              <div className="min-w-0 space-y-0.5">
                <p className="font-mono text-sm font-medium">
                  {summary.port}
                  <span className="text-muted-foreground">
                    /{summary.protocol.toLowerCase()}
                  </span>
                </p>
                <p className="text-xs text-muted-foreground">
                  First {formatHistorySeen(summary.firstSeen)} · Last{" "}
                  {formatHistorySeen(summary.lastSeen)}
                </p>
              </div>
              <Badge variant="secondary" className="shrink-0 font-mono text-[10px]">
                {summary.eventCount}
              </Badge>
            </button>
            {selected && (
              <div className="border-t bg-muted/10 px-2.5 py-2.5">
                <PortHistoryTimeline port={summary.port} />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
