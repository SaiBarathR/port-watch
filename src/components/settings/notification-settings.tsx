import { useState } from "react";
import { ChevronDownIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { SettingRow, SettingSection } from "@/components/settings/setting-row";
import {
  MUTE_DURATIONS,
  changeToastStatus,
  formatMutedUntil,
} from "@/lib/change-toasts";
import {
  setChangeToastsMutedUntil,
  setShowChangeToasts,
} from "@/lib/settings-actions";
import { updateSettings } from "@/lib/settings-store";
import { parsePort, type AppSettings } from "@/lib/types";

export function NotificationSettings({ settings }: { settings: AppSettings }) {
  const [watchedPortInput, setWatchedPortInput] = useState("");
  // A mute runs out with the clock, not with a state change, so the clock
  // is read on every render.
  // eslint-disable-next-line react-hooks/purity
  const toastsMuted = changeToastStatus(settings, Date.now()) === "muted";

  const addWatchedPort = () => {
    const port = parsePort(watchedPortInput);
    if (port === null) {
      return;
    }
    if (!settings.watchedPorts.includes(port)) {
      updateSettings({
        watchedPorts: [...settings.watchedPorts, port].sort((a, b) => a - b),
      });
    }
    setWatchedPortInput("");
  };

  return (
    <SettingSection title="Notifications">
      <SettingRow
        label="Port change toasts"
        description="A note in the window when a port is taken or freed."
      >
        {(ids) => (
          <Switch
            id={ids.control}
            checked={settings.showChangeToasts}
            onCheckedChange={setShowChangeToasts}
          />
        )}
      </SettingRow>

      <SettingRow
        label="Mute toasts"
        description={
          toastsMuted && settings.changeToastsMutedUntil !== null
            ? `Muted until ${formatMutedUntil(settings.changeToastsMutedUntil)}.`
            : "Pause port change toasts for a while."
        }
      >
        {(ids) => (
          // A menu of actions rather than a Select (whose closed trigger
          // picks an option on any keypress), kept mounted in both states
          // so keyboard focus returns to it.
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                aria-describedby={ids.label}
                type="button"
                size="sm"
                variant="outline"
                className="w-full min-w-[8.5rem] justify-between font-normal sm:w-[8.5rem]"
                disabled={!settings.showChangeToasts}
              >
                {toastsMuted ? "Muted" : "Not muted"}
                <ChevronDownIcon className="opacity-50" />
              </Button>
            </DropdownMenuTrigger>
            {/* Above the toast stack, which overlaps the dialog in a narrow window. */}
            <DropdownMenuContent align="end" className="z-[1000000000]">
              {toastsMuted && (
                <DropdownMenuItem
                  onClick={() => setChangeToastsMutedUntil(null)}
                >
                  Unmute
                </DropdownMenuItem>
              )}
              {MUTE_DURATIONS.map((duration) => (
                <DropdownMenuItem
                  key={duration.ms}
                  onClick={() =>
                    setChangeToastsMutedUntil(Date.now() + duration.ms)
                  }
                >
                  Mute for {duration.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </SettingRow>

      <SettingRow
        label="Watched port alerts"
        description="A desktop notification when a watched port is taken, freed, or changes hands."
      >
        {(ids) => (
          <Switch
            id={ids.control}
            checked={settings.watchedPortNotifications}
            onCheckedChange={(watchedPortNotifications) =>
              updateSettings({ watchedPortNotifications })
            }
          />
        )}
      </SettingRow>

      <SettingRow
        stacked
        label="Watched ports"
        description="Add a port here, or use Watch Port in a row's menu."
      >
        {(ids) => (
          <>
            <div className="flex gap-2">
              <Input
                id={ids.control}
                inputMode="numeric"
                placeholder="e.g. 3000"
                value={watchedPortInput}
                onChange={(event) =>
                  setWatchedPortInput(event.target.value.replace(/\D/g, ""))
                }
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    addWatchedPort();
                  }
                }}
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={addWatchedPort}
              >
                Add
              </Button>
            </div>
            {settings.watchedPorts.length > 0 && (
              <ul className="mt-2 flex flex-wrap gap-1.5">
                {settings.watchedPorts.map((port) => (
                  <li key={port}>
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      className="h-7 gap-1 font-mono text-xs"
                      aria-label={`Stop watching port ${port}`}
                      onClick={() =>
                        updateSettings({
                          watchedPorts: settings.watchedPorts.filter(
                            (watched) => watched !== port,
                          ),
                        })
                      }
                    >
                      {port}
                      <XIcon className="size-3" />
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </SettingRow>
    </SettingSection>
  );
}
