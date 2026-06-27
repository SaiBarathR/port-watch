import { useEffect, useState } from "react";
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
import type { PortProcess } from "@/lib/types";
import { formatPorts } from "@/lib/types";
import { basename } from "@/lib/utils";

interface DeleteTarget {
  process: PortProcess;
  mode: "trash" | "permanent";
}

interface DeleteDialogProps {
  target: DeleteTarget | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onComplete: () => void;
}

export function DeleteDialog({
  target,
  open,
  onOpenChange,
  onComplete,
}: DeleteDialogProps) {
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);

  const path =
    target?.process.working_directory ||
    target?.process.script_path ||
    target?.process.executable_path ||
    "";

  const folderBasename = basename(path);

  useEffect(() => {
    if (!open) {
      setConfirmation("");
      setBusy(false);
    }
  }, [open]);

  const handleDelete = async () => {
    if (!target || !path) return;

    setBusy(true);
    try {
      await invoke("stop_process", { pid: target.process.pid });

      if (target.mode === "trash") {
        await invoke("move_to_trash", { path });
        toast.success("Moved folder to Trash");
      } else {
        await invoke("delete_permanently", { path, confirmation });
        toast.success("Folder deleted permanently");
      }

      onOpenChange(false);
      onComplete();
    } catch (err) {
      toast.error(String(err));
    } finally {
      setBusy(false);
    }
  };

  if (!target) return null;

  const isPermanent = target.mode === "permanent";
  const canDelete =
    !isPermanent || confirmation === folderBasename;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
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
          <AlertTitle>Destructive action</AlertTitle>
          <AlertDescription>
            Protected system paths under /System, /usr, /bin, /sbin, and /Library
            cannot be deleted.
          </AlertDescription>
        </Alert>

        {isPermanent && (
          <div className="flex flex-col gap-2">
            <Label htmlFor="confirm-basename">
              Type <span className="font-mono font-semibold">{folderBasename}</span> to
              confirm
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
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            disabled={busy || !canDelete || !path}
            onClick={() => void handleDelete()}
          >
            {isPermanent ? "Delete Permanently" : "Move to Trash"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
