import { useEffect, useState } from "react";
import { toast } from "sonner";
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
import { isMacOS } from "@/hooks/use-liquid-glass";
import {
  dismissCliInstallPrompt,
  fetchCliInstallStatus,
  installCliToPath,
  isCliInstallPromptDismissed,
} from "@/lib/cli-install";

export function CliInstallPrompt() {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isMacOS() || isCliInstallPromptDismissed()) {
      return;
    }

    let cancelled = false;

    void fetchCliInstallStatus().then((status) => {
      if (cancelled || !status || status.pointsToApp) {
        return;
      }
      setOpen(true);
    });

    return () => {
      cancelled = true;
    };
  }, []);

  const handleDismiss = () => {
    dismissCliInstallPrompt();
    setOpen(false);
  };

  const handleInstall = async () => {
    setBusy(true);
    try {
      await installCliToPath();
      dismissCliInstallPrompt();
      setOpen(false);
      toast.success("Command-line tool installed", {
        description: "Run port-watch check 3000 from Terminal.",
      });
    } catch (err) {
      toast.error("Could not install CLI", {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setBusy(false);
    }
  };

  if (!isMacOS()) {
    return null;
  }

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          handleDismiss();
        }
      }}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Install command-line tool?</AlertDialogTitle>
          <AlertDialogDescription>
            Add <span className="font-mono text-foreground">port-watch</span> to
            your PATH so you can run{" "}
            <span className="font-mono text-foreground">port-watch check 3000</span>{" "}
            from Terminal and CI scripts. macOS may ask for your password to write
            to <span className="font-mono text-foreground">/usr/local/bin</span>.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy} onClick={handleDismiss}>
            Not now
          </AlertDialogCancel>
          <Button disabled={busy} onClick={() => void handleInstall()}>
            {busy ? "Installing…" : "Install"}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
