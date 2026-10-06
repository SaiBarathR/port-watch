import { TerminalIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { SettingRow, SettingSection } from "@/components/settings/setting-row";
import {
  installCli,
  uninstallCli,
  useCliInstall,
} from "@/hooks/use-cli-install";
import { cliInstallPrivilegeHint } from "@/lib/platform";
import { updateSettings } from "@/lib/settings-store";
import type { AppSettings } from "@/lib/types";

export function IntegrationSettings({ settings }: { settings: AppSettings }) {
  const cli = useCliInstall();
  const installed = cli.status?.pointsToApp ?? false;

  return (
    <SettingSection title="Integrations">
      <SettingRow
        label="Editor"
        description="What Open in Editor opens a project in."
      >
        {(ids) => (
          <Select
            value={settings.preferredEditor}
            onValueChange={(value) =>
              updateSettings({
                preferredEditor: value as AppSettings["preferredEditor"],
              })
            }
          >
            <SelectTrigger
              id={ids.control}
              size="sm"
              className="w-full min-w-[8.5rem] sm:w-[8.5rem]"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="cursor">Cursor</SelectItem>
              <SelectItem value="code">VS Code</SelectItem>
            </SelectContent>
          </Select>
        )}
      </SettingRow>

      <SettingRow
        label="Use HTTPS for localhost"
        description="Open in Browser and Copy URL use https://localhost instead of http://."
      >
        {(ids) => (
          <Switch
            id={ids.control}
            checked={settings.useHttpsForLocalhost}
            onCheckedChange={(useHttpsForLocalhost) =>
              updateSettings({ useHttpsForLocalhost })
            }
          />
        )}
      </SettingRow>

      <SettingRow
        label="Command-line tool"
        description={
          <>
            <span className="font-mono">port-watch check 3000</span> from a
            terminal or a script.{" "}
            {installed
              ? `Installed at ${cli.status?.linkPath}.`
              : cli.status?.installed
                ? `Another port-watch is at ${cli.status.linkPath}.`
                : cliInstallPrivilegeHint()}
          </>
        }
      >
        {(ids) => (
          <Button
            aria-describedby={ids.label}
            type="button"
            size="sm"
            variant="outline"
            className="min-w-[8.5rem]"
            disabled={cli.busy || cli.status === null}
            onClick={() => void (installed ? uninstallCli() : installCli())}
          >
            {!installed && <TerminalIcon />}
            {cli.busy ? "Working…" : installed ? "Uninstall" : "Install"}
          </Button>
        )}
      </SettingRow>
    </SettingSection>
  );
}
