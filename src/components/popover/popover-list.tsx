import { TooltipProvider } from "@/components/ui/tooltip";
import type { PortProcess, RowChangeKind } from "@/lib/types";
import { PopoverRow } from "./popover-row";

interface PopoverListProps {
  processes: PortProcess[];
  rowChanges: Map<number, RowChangeKind>;
  allowSystemProcessActions: boolean;
  useHttpsForLocalhost: boolean;
  portLookupEmpty: boolean;
  exactPortQuery: number | null;
  loading: boolean;
  onStop: (process: PortProcess) => void;
}

export function PopoverList({
  processes,
  rowChanges,
  allowSystemProcessActions,
  useHttpsForLocalhost,
  portLookupEmpty,
  exactPortQuery,
  loading,
  onStop,
}: PopoverListProps) {
  const canStop = (process: PortProcess) =>
    !process.is_system_service || allowSystemProcessActions;

  if (portLookupEmpty && exactPortQuery !== null) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center px-6 py-8 text-center">
        <p className="text-sm font-medium">Nothing listening on port {exactPortQuery}</p>
        <p className="mt-1 text-xs text-muted-foreground">
          No process is bound to this port right now.
        </p>
      </div>
    );
  }

  if (!loading && processes.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center px-6 py-8 text-center">
        <p className="text-sm font-medium">No dev servers listening</p>
        <p className="mt-1 text-xs text-muted-foreground">
          User processes with open ports will appear here.
        </p>
      </div>
    );
  }

  const sorted = [...processes].sort(
    (a, b) => (a.ports[0]?.port ?? 0) - (b.ports[0]?.port ?? 0),
  );

  return (
    <TooltipProvider delayDuration={300}>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {sorted.map((process) => (
          <PopoverRow
            key={process.pid}
            process={process}
            change={rowChanges.get(process.pid)}
            canStop={canStop(process)}
            useHttpsForLocalhost={useHttpsForLocalhost}
            onStop={onStop}
          />
        ))}
      </div>
    </TooltipProvider>
  );
}
