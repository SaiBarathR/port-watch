import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { toast } from "sonner";
import { DeleteDialog } from "@/components/delete-dialog";
import { PortHistoryDialog } from "@/components/port-history-dialog";
import { StopDialog } from "@/components/stop-dialog";
import { useRefreshPause } from "@/lib/refresh-pause";
import { focusRow, focusTabStopRow } from "@/lib/row-focus";
import { useSettings } from "@/lib/settings-store";
import { processesOnPort, type PortProcess } from "@/lib/types";

/** What can be done to a listed process, wherever the request comes from. */
export interface ProcessActions {
  /** False for a system service while those are locked, and for a listener
   * whose owner the scan could not see: there is no process to act on. */
  canStop: (process: PortProcess) => boolean;
  /** Asks, then stops the processes given. */
  stop: (targets: PortProcess[], title?: string, description?: string) => void;
  /**
   * Asks, then stops everything holding the port that can be stopped. A
   * socket can be shared (a reloader and its worker, a pre-forked server),
   * so freeing a port means stopping all of them. `first` leads the list.
   */
  freePort: (port: number, first?: PortProcess) => void;
  /** Asks, then stops the process and removes its project folder. */
  remove: (process: PortProcess, mode: "trash" | "permanent") => void;
  /** One port's history, or every port's when none is given. */
  showHistory: (port?: number) => void;
}

const ProcessActionsContext = createContext<ProcessActions | null>(null);

export function useProcessActions(): ProcessActions {
  const actions = useContext(ProcessActionsContext);
  if (!actions) {
    throw new Error("useProcessActions needs a ProcessActionsProvider");
  }
  return actions;
}

interface StopRequest {
  targets: PortProcess[];
  title?: string;
  description?: string;
}

interface ProcessActionsProviderProps {
  /** Every listener, shown or not: freeing a port reaches past the filter. */
  processes: PortProcess[];
  /** Called after a stop or a removal was tried, whether or not it worked. */
  onChanged: () => void;
  children: ReactNode;
}

/**
 * The one owner of the stop, delete and history dialogs. The toolbar's port
 * lookup and the table's rows each had a stop dialog of their own, and chose
 * what "free this port" stops differently.
 */
export function ProcessActionsProvider({
  processes,
  onChanged,
  children,
}: ProcessActionsProviderProps) {
  const { allowSystemProcessActions } = useSettings();
  const [stopRequest, setStopRequest] = useState<StopRequest | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{
    process: PortProcess;
    mode: "trash" | "permanent";
  } | null>(null);
  const [history, setHistory] = useState<number | "all" | null>(null);

  // A dialog about a process that is no longer listed has nothing left to
  // act on. A stop that was refused because the row was out of date, or a
  // delete that stopped the process and then could not remove its folder,
  // would otherwise leave one open that can only fail again.
  const listed = (process: PortProcess) =>
    processes.some((item) => item.id === process.id);
  if (stopRequest !== null && !stopRequest.targets.some(listed)) {
    setStopRequest(null);
  }
  if (deleteTarget !== null && !listed(deleteTarget.process)) {
    setDeleteTarget(null);
  }

  const dialogOpen =
    stopRequest !== null || deleteTarget !== null || history !== null;
  // Rows must not move behind a dialog that is about them.
  useRefreshPause("process-dialog", dialogOpen);

  // A dialog hands the keyboard back to the button that opened it, and these
  // have none: they open from menus and shortcuts. So where the keyboard was
  // is noted when one is asked for, and it is put back when the dialog
  // closes, on the row it was about if the menu item itself is gone.
  const returnFocusRef = useRef<{ element: Element | null; rowId?: string }>({
    element: null,
  });
  const noteFocus = useCallback((rowId?: string) => {
    returnFocusRef.current = { element: document.activeElement, rowId };
  }, []);
  const wasOpenRef = useRef(false);
  useEffect(() => {
    const closed = wasOpenRef.current && !dialogOpen;
    wasOpenRef.current = dialogOpen;
    if (!closed) {
      return;
    }
    const { element, rowId } = returnFocusRef.current;
    const timer = window.setTimeout(() => {
      if (element instanceof HTMLElement && element.isConnected) {
        element.focus();
      } else if (!rowId || !focusRow(rowId)) {
        focusTabStopRow();
      }
    }, 0);
    return () => window.clearTimeout(timer);
  }, [dialogOpen]);

  // Read through a ref, so the actions keep their identity from one scan to
  // the next and the rows that hold them do not render again.
  const processesRef = useRef(processes);
  useEffect(() => {
    processesRef.current = processes;
  }, [processes]);

  const canStop = useCallback(
    (process: PortProcess) =>
      process.pid !== 0 &&
      (!process.is_system_service || allowSystemProcessActions),
    [allowSystemProcessActions],
  );

  const actions = useMemo<ProcessActions>(
    () => ({
      canStop,
      stop: (targets, title, description) => {
        if (targets.length > 0) {
          noteFocus(targets.length === 1 ? targets[0].id : undefined);
          setStopRequest({ targets, title, description });
        }
      },
      freePort: (port, first) => {
        const holders = processesOnPort(processesRef.current, port).filter(
          (holder) => holder.id !== first?.id && canStop(holder),
        );
        const targets = first ? [first, ...holders] : holders;
        if (targets.length === 0) {
          toast.error(`Nothing on port ${port} can be stopped from here`, {
            description:
              "What holds it is a system service, or belongs to another user.",
          });
          return;
        }
        noteFocus(first?.id);
        setStopRequest({
          targets,
          title: `Free port ${port}?`,
          description:
            targets.length === 1
              ? `Stop ${targets[0].name} (PID ${targets[0].pid}) to free port ${port}.`
              : `Stop ${targets.length} processes to free port ${port}.`,
        });
      },
      remove: (process, mode) => {
        noteFocus(process.id);
        setDeleteTarget({ process, mode });
      },
      showHistory: (port) => {
        noteFocus();
        setHistory(port ?? "all");
      },
    }),
    [canStop, noteFocus],
  );

  return (
    <ProcessActionsContext.Provider value={actions}>
      {children}

      <StopDialog
        processes={stopRequest?.targets ?? []}
        open={stopRequest !== null}
        onOpenChange={(open) => !open && setStopRequest(null)}
        title={stopRequest?.title}
        description={stopRequest?.description}
        requireDoubleConfirm={
          allowSystemProcessActions &&
          (stopRequest?.targets.some((process) => process.is_system_service) ??
            false)
        }
        onComplete={onChanged}
      />

      <DeleteDialog
        target={deleteTarget}
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        allowSystemProcessActions={allowSystemProcessActions}
        onComplete={onChanged}
      />

      <PortHistoryDialog show={history} onClose={() => setHistory(null)} />
    </ProcessActionsContext.Provider>
  );
}
