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
import { formatPorts } from "@/lib/types";

interface StopDialogProps {
  processes: PortProcess[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  requireDoubleConfirm: boolean;
  onStopped: () => void;
  title?: string;
  description?: string;
}

export function StopDialog({
  processes,
  open,
  onOpenChange,
  requireDoubleConfirm,
  onStopped,
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
    let stopped = 0;

    for (const process of processes) {
      try {
        await invoke("stop_process", { pid: process.pid });
        stopped += 1;
      } catch (err) {
        failures.push(`${process.name} (${process.pid}): ${String(err)}`);
      }
    }

    if (stopped > 0) {
      toast.success(
        stopped === 1
          ? `Stopped ${processes[0].name} (PID ${processes[0].pid})`
          : `Stopped ${stopped} process${stopped === 1 ? "" : "es"}`,
      );
      handleOpenChange(false);
      onStopped();
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
  const hasSystemService = processes.some((process) => process.is_system_service);
  const dialogTitle =
    title ??
    (single
      ? `Stop ${single.name} on port ${formatPorts(single.ports)}?`
      : `Stop ${processes.length} selected processes?`);
  const dialogDescription =
    description ??
    (single
      ? `This sends SIGTERM to PID ${single.pid}, then SIGKILL after 2 seconds if the process is still running.`
      : "Each process receives SIGTERM, then SIGKILL after 2 seconds if still running.");

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
              <li key={process.pid} className="truncate font-mono">
                {formatPorts(process.ports)} — {process.name} (PID {process.pid})
              </li>
            ))}
          </ul>
        )}

        {requireDoubleConfirm && hasSystemService && confirmStep && (
          <Alert variant="destructive">
            <AlertTriangleIcon />
            <AlertTitle>System service warning</AlertTitle>
            <AlertDescription>
              You are about to stop one or more system services. This may affect
              macOS functionality. Confirm again to proceed.
            </AlertDescription>
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
