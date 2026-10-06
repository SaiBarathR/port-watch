import { useState } from "react";
import { TerminalIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCliInstall } from "@/hooks/use-cli-install";
import {
  dismissCliInstallPrompt,
  isCliInstallPromptDismissed,
} from "@/lib/cli-install";
import { getPlatform } from "@/lib/platform";

const SUPPORTED_PLATFORMS = new Set(["macos", "linux", "windows"]);

/**
 * Offers the `port-watch` command once, in a strip under the table. It was
 * a dialog on first launch, which stood between a new user and the app to
 * ask about something they had not seen a use for yet.
 */
export function CliInstallBanner() {
  const supported = SUPPORTED_PLATFORMS.has(getPlatform());
  const [dismissed, setDismissed] = useState(isCliInstallPromptDismissed);
  const cli = useCliInstall(supported && !dismissed);

  if (!supported || dismissed || !cli.status || cli.status.pointsToApp) {
    return null;
  }

  const dismiss = () => {
    dismissCliInstallPrompt();
    setDismissed(true);
  };

  return (
    <aside
      aria-label="Command-line tool"
      className="flex items-center gap-3 rounded-lg border bg-muted/30 py-1.5 pr-1.5 pl-3 text-sm"
    >
      <TerminalIcon
        aria-hidden
        className="size-4 shrink-0 text-muted-foreground"
      />
      <p className="min-w-0 flex-1 truncate">
        <span className="font-medium">Check ports from your terminal.</span>{" "}
        <span className="text-muted-foreground">
          Install the command-line tool to run{" "}
          <span className="font-mono">port-watch check 3000</span> there and in
          scripts.
        </span>
      </p>
      <Button
        size="sm"
        variant="outline"
        className="h-7 shrink-0"
        disabled={cli.busy}
        onClick={() =>
          void cli.install().then((installed) => installed && dismiss())
        }
      >
        {cli.busy ? "Installing…" : "Install"}
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className="size-7 shrink-0"
        aria-label="Dismiss"
        disabled={cli.busy}
        onClick={dismiss}
      >
        <XIcon />
      </Button>
    </aside>
  );
}
