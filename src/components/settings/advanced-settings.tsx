import { TriangleAlertIcon } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { SettingRow, SettingSection } from "@/components/settings/setting-row";
import { platformLabel } from "@/lib/platform";
import { updateSettings } from "@/lib/settings-store";
import type { AppSettings } from "@/lib/types";

export function AdvancedSettings({ settings }: { settings: AppSettings }) {
  return (
    <SettingSection title="Advanced">
      <SettingRow
        label="Include UDP"
        description="Also list UDP sockets. Most of what listens on UDP belongs to the system."
      >
        {(ids) => (
          <Switch
            id={ids.control}
            checked={settings.includeUdp}
            onCheckedChange={(includeUdp) => updateSettings({ includeUdp })}
          />
        )}
      </SettingRow>

      <SettingRow
        label="Allow actions on system services"
        description={
          <span className="flex gap-1.5">
            <TriangleAlertIcon
              aria-hidden
              className="mt-px size-3.5 shrink-0 text-amber-600 dark:text-amber-400"
            />
            <span>
              Lets Stop and Move to Trash act on what {platformLabel()} itself
              runs. Stopping one can break part of the system until it restarts,
              so a stop still asks twice.
            </span>
          </span>
        }
      >
        {(ids) => (
          <Switch
            id={ids.control}
            checked={settings.allowSystemProcessActions}
            onCheckedChange={(allowSystemProcessActions) =>
              updateSettings({ allowSystemProcessActions })
            }
          />
        )}
      </SettingRow>
    </SettingSection>
  );
}
