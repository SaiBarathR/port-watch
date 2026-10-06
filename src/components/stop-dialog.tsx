import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangleIcon } from "lucide-react";
import { toast } from "sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import type { PortProcess } from "@/lib/types";
import { formatPorts, oldestFirst } from "@/lib/types";
import {
  stopMultipleProcessDescription,
  stopProcessDescription,
  systemStopWarning,
} from "@/lib/platform";

interface StopDialogProps {
  processes: PortProcess[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  requireDoubleConfirm: boolean;
  /** Called once the stops have been tried, whether or not they worked. */
  onComplete: () => void;
  title?: string;
  description?: string;
}

export function StopDialog({
  processes,
  open,
  onOpenChange,
  requireDoubleConfirm,
  onComplete,
  title,
  description,
}: StopDialogProps) {
  const [confirmStep, setConfirmStep] = useState(false);
  const [busy, setBusy] = useState(false);

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setConfirmStep(false);
    }
    onOpenChange(next);
  };

  const stopProcesses = async () => {
    if (processes.length === 0) {
      return;
    }

    setBusy(true);
    const failures: string[] = [];
    const stopped: PortProcess[] = [];

    const ordered = oldestFirst(processes);

    for (const process of ordered) {
      try {
        await invoke("stop_process", {
          pid: process.pid,
          expectedName: process.name,
          expectedStartedAt: process.started_at,
        });
        stopped.push(process);
      } catch (err) {
        failures.push(`${process.name} (${process.pid}): ${String(err)}`);
      }
    }

    if (stopped.length > 0) {
      toast.success(
        stopped.length === 1
          ? `Stopped ${stopped[0].name} (PID ${stopped[0].pid})`
          : `Stopped ${stopped.length} processes`,
      );
      handleOpenChange(false);
    }

    if (failures.length > 0) {
      toast.error(
        failures.length === 1
          ? failures[0]
          : `Failed to stop ${failures.length} processes`,
        {
          description:
            failures.length > 1 ? failures.slice(0, 3).join("\n") : undefined,
        },
      );
    }

    setBusy(false);
    // Also when nothing could be stopped: a refusal usually means the row was
    // out of date (the process had gone, or its PID had passed to another),
    // and the list should show what is there now.
    onComplete();
  };

  const handleConfirm = () => {
    if (requireDoubleConfirm && !confirmStep) {
      setConfirmStep(true);
      return;
    }
    void stopProcesses();
  };

  if (processes.length === 0) {
    return null;
  }

  const single = processes.length === 1 ? processes[0] : null;
  const hasSystemService = processes.some(
    (process) => process.is_system_service,
  );
  const dialogTitle =
    title ??
    (single
      ? `Stop ${single.name} on port ${formatPorts(single.ports)}?`
      : `Stop ${processes.length} selected processes?`);
  const dialogDescription =
    description ??
    (single
      ? stopProcessDescription(single.pid)
      : stopMultipleProcessDescription());

  return (
    <AlertDialog open={open} onOpenChange={handleOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{dialogTitle}</AlertDialogTitle>
          <AlertDialogDescription>{dialogDescription}</AlertDialogDescription>
        </AlertDialogHeader>

        {processes.length > 1 && (
          <ul className="max-h-40 space-y-1 overflow-y-auto rounded-md border bg-muted/20 p-3 text-sm">
            {processes.map((process) => (
              <li key={process.id} className="truncate font-mono">
                {formatPorts(process.ports)} — {process.name} (PID {process.pid}
                )
              </li>
            ))}
          </ul>
        )}

        {requireDoubleConfirm && hasSystemService && confirmStep && (
          <Alert variant="destructive">
            <AlertTriangleIcon />
            <AlertTitle>System service warning</AlertTitle>
            <AlertDescription>{systemStopWarning()}</AlertDescription>
          </Alert>
        )}

        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
          <Button disabled={busy} variant="destructive" onClick={handleConfirm}>
            {requireDoubleConfirm && hasSystemService && !confirmStep
              ? "Continue"
              : processes.length === 1
                ? "Stop Process"
                : `Stop ${processes.length} Processes`}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
