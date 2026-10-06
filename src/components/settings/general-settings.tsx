import { MonitorIcon, MoonIcon, MoonStarIcon, SunIcon } from "lucide-react";
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
import type { ThemeMode } from "@/hooks/use-theme";
import { isMacOS } from "@/lib/platform";
import { updateSettings } from "@/lib/settings-store";
import type { AppSettings, RefreshInterval } from "@/lib/types";
import { cn } from "@/lib/utils";

const THEME_OPTIONS: {
  value: ThemeMode;
  label: string;
  icon: typeof SunIcon;
}[] = [
  { value: "light", label: "Light", icon: SunIcon },
  { value: "dark-grey", label: "Grey", icon: MoonIcon },
  { value: "dark-oled", label: "Dark", icon: MoonStarIcon },
  { value: "system", label: "System", icon: MonitorIcon },
];

interface GeneralSettingsProps {
  settings: AppSettings;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
}

export function GeneralSettings({
  settings,
  theme,
  onThemeChange,
}: GeneralSettingsProps) {
  return (
    <SettingSection title="General">
      <SettingRow
        label="Theme"
        description="Light, grey, dark, or match system."
      >
        {(ids) => (
          <div
            className="flex items-center rounded-lg border bg-muted/40 p-0.5"
            role="group"
            aria-labelledby={ids.label}
          >
            {THEME_OPTIONS.map(({ value, label, icon: Icon }) => (
              <Button
                key={value}
                type="button"
                variant="ghost"
                size="icon"
                className={cn(
                  "size-7",
                  theme === value &&
                    "bg-background text-foreground shadow-xs hover:bg-background",
                )}
                aria-pressed={theme === value}
                aria-label={label}
                onClick={() => onThemeChange(value)}
              >
                <Icon
                  className="size-3.5 shrink-0"
                  strokeWidth={1.75}
                  aria-hidden
                />
              </Button>
            ))}
          </div>
        )}
      </SettingRow>

      <SettingRow
        label="Auto-refresh"
        description="How often to look for listening ports."
      >
        {(ids) => (
          <Select
            value={String(settings.refreshIntervalMs)}
            onValueChange={(value) =>
              updateSettings({
                refreshIntervalMs: Number(value) as RefreshInterval,
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
              <SelectItem value="3000">Every 3 s</SelectItem>
              <SelectItem value="10000">Every 10 s</SelectItem>
              <SelectItem value="0">Off</SelectItem>
            </SelectContent>
          </Select>
        )}
      </SettingRow>

      <SettingRow
        label="Group by project"
        description="Keep processes from the same project folder together."
      >
        {(ids) => (
          <Switch
            id={ids.control}
            checked={settings.groupByDirectory}
            onCheckedChange={(groupByDirectory) =>
              updateSettings({ groupByDirectory })
            }
          />
        )}
      </SettingRow>

      {isMacOS() && (
        <SettingRow
          label="Menu bar mode"
          description="Run from the menu bar only, with no Dock icon. Turning it on closes this window; the menu bar icon opens it again."
        >
          {(ids) => (
            <Switch
              id={ids.control}
              checked={settings.menuBarMode}
              onCheckedChange={(menuBarMode) => updateSettings({ menuBarMode })}
            />
          )}
        </SettingRow>
      )}
    </SettingSection>
  );
}
