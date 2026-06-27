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
import {
  cliInstallPathHint,
  cliInstallPrivilegeHint,
  getPlatform,
} from "@/lib/platform";
import {
  dismissCliInstallPrompt,
  fetchCliInstallStatus,
  installCliToPath,
  isCliInstallPromptDismissed,
} from "@/lib/cli-install";

const SUPPORTED_PLATFORMS = new Set(["macos", "linux", "windows"]);

export function CliInstallPrompt() {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const platform = getPlatform();
    if (!SUPPORTED_PLATFORMS.has(platform) || isCliInstallPromptDismissed()) {
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
        description: "Run port-watch check 3000 from your terminal.",
      });
    } catch (err) {
      toast.error("Could not install CLI", {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setBusy(false);
    }
  };

  if (!SUPPORTED_PLATFORMS.has(getPlatform())) {
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
            from terminal and CI scripts. {cliInstallPrivilegeHint()}{" "}
            Target:{" "}
            <span className="font-mono text-foreground">{cliInstallPathHint()}</span>.
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
