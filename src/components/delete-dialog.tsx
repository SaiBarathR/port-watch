import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangleIcon } from "lucide-react";
import { toast } from "sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { deletableFoldersDescription } from "@/lib/platform";
import type { PortProcess } from "@/lib/types";
import { formatPorts, pinPath } from "@/lib/types";
import { basename } from "@/lib/utils";

interface DeleteTarget {
  process: PortProcess;
  mode: "trash" | "permanent";
}

interface DeleteDialogProps {
  target: DeleteTarget | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  allowSystemProcessActions: boolean;
  onComplete: () => void;
}

export function DeleteDialog({
  target,
  open,
  onOpenChange,
  allowSystemProcessActions,
  onComplete,
}: DeleteDialogProps) {
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);

  const path = target ? pinPath(target.process) : "";
  const folderBasename = basename(path);
  const blockedReason = target?.process.delete_blocked ?? null;
  const canDelete =
    !!target &&
    !!path &&
    blockedReason === null &&
    (!target.process.is_system_service || allowSystemProcessActions);

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setConfirmation("");
    }
    onOpenChange(next);
  };

  const handleDelete = async () => {
    if (!target || !path || !canDelete) return;

    setBusy(true);
    try {
      // One command: the backend checks the folder before it stops anything.
      await invoke("delete_project", {
        pid: target.process.pid,
        expectedName: target.process.name,
        path,
        mode: target.mode,
        confirmation: target.mode === "permanent" ? confirmation : null,
      });
      toast.success(
        target.mode === "trash"
          ? "Moved folder to Trash"
          : "Folder deleted permanently",
      );
      handleOpenChange(false);
    } catch (err) {
      toast.error(String(err));
    } finally {
      setBusy(false);
      // Also after a failure: the process may have been stopped even though
      // its folder could not be deleted.
      onComplete();
    }
  };

  if (!target) return null;

  const isPermanent = target.mode === "permanent";
  const canSubmit =
    canDelete && (!isPermanent || confirmation === folderBasename);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {isPermanent ? "Delete permanently" : "Move to Trash"}
          </DialogTitle>
          <DialogDescription>
            {isPermanent
              ? "This action cannot be undone. The running process will be stopped first."
              : "This will move the folder to Trash. The running process will be stopped first."}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-3 text-sm">
          <p>
            <span className="text-muted-foreground">Process:</span>{" "}
            {target.process.name} · {formatPorts(target.process.ports)}
          </p>
          <p className="break-all">
            <span className="text-muted-foreground">Path:</span> {path}
          </p>
        </div>

        <Alert variant="destructive">
          <AlertTriangleIcon />
          <AlertTitle>
            {blockedReason
              ? "This folder cannot be deleted"
              : "Destructive action"}
          </AlertTitle>
          <AlertDescription>
            {blockedReason ?? deletableFoldersDescription()}
          </AlertDescription>
        </Alert>

        {isPermanent && (
          <div className="flex flex-col gap-2">
            <Label htmlFor="confirm-basename">
              Type{" "}
              <span className="font-mono font-semibold">{folderBasename}</span>{" "}
              to confirm
            </Label>
            <Input
              id="confirm-basename"
              value={confirmation}
              onChange={(e) => setConfirmation(e.target.value)}
              placeholder={folderBasename}
              autoComplete="off"
            />
          </div>
        )}

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => handleOpenChange(false)}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button
            variant="destructive"
            disabled={busy || !canSubmit}
            onClick={() => void handleDelete()}
          >
            {isPermanent ? "Delete Permanently" : "Move to Trash"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
