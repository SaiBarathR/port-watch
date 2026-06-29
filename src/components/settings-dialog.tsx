import { useEffect, useState, type ReactNode } from "react";
import {
  MonitorIcon,
  MoonIcon,
  MoonStarIcon,
  SunIcon,
  TerminalIcon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { ThemeMode } from "@/hooks/use-theme";
import { cliInstallPrivilegeHint, isMacOS } from "@/lib/platform";
import { PortHistoryList } from "@/components/port-history-timeline";
import { clearPortHistory } from "@/lib/port-history";
import {
  fetchCliInstallStatus,
  installCliToPath,
  uninstallCliFromPath,
  type CliInstallStatus,
} from "@/lib/cli-install";
import {
  GLASS_BLUR_OPTIONS,
  GLASS_TRANSLUCENCY_OPTIONS,
  type AppSettings,
  type RefreshInterval,
} from "@/lib/types";
import { cn } from "@/lib/utils";

const THEME_OPTIONS: {
  value: ThemeMode;
  label: string;
  icon: typeof SunIcon;
}[] = [
  { value: "light", label: "Light", icon: SunIcon },
  { value: "dark-grey", label: "Grey", icon: MoonIcon },
  { value: "dark-oled", label: "OLED", icon: MoonStarIcon },
  { value: "system", label: "System", icon: MonitorIcon },
];

function SettingRow({
  label,
  description,
  htmlFor,
  children,
  stacked = false,
}: {
  label: string;
  description?: string;
  htmlFor?: string;
  children?: ReactNode;
  stacked?: boolean;
}) {
  return (
    <div
      className={cn(
        "gap-3 py-3",
        stacked || !children
          ? "flex flex-col"
          : "grid grid-cols-1 items-center sm:grid-cols-[minmax(0,1fr)_auto]",
      )}
    >
      <div className="min-w-0 space-y-0.5">
        <Label htmlFor={htmlFor} className="text-sm font-medium">
          {label}
        </Label>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      {children ? (
        <div className={cn("min-w-0", !stacked && "sm:justify-self-end")}>{children}</div>
      ) : null}
    </div>
  );
}

function SettingSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="min-w-0">
      <h3 className="mb-1.5 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {title}
      </h3>
      <div className="divide-y overflow-hidden rounded-lg border bg-muted/20 px-4">
        {children}
      </div>
    </section>
  );
}

interface SettingsDialogProps {
  settings: AppSettings;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  onAllowSystemActionsChange: (allow: boolean) => void;
  onRefreshIntervalChange: (interval: RefreshInterval) => void;
  onPreferredEditorChange: (editor: AppSettings["preferredEditor"]) => void;
  onGroupByDirectoryChange: (group: boolean) => void;
  onShowChangeToastsChange: (show: boolean) => void;
  onWatchedPortNotificationsChange: (enabled: boolean) => void;
  onWatchedPortsChange: (ports: number[]) => void;
  onIncludeUdpChange: (include: boolean) => void;
  onUseHttpsForLocalhostChange: (useHttps: boolean) => void;
  onLiquidGlassChange: (enabled: boolean) => void;
  onGlassTranslucencyChange: (value: AppSettings["glassTranslucency"]) => void;
  onGlassBlurChange: (value: AppSettings["glassBlur"]) => void;
  onGlassTintChange: (enabled: boolean) => void;
  trigger: ReactNode;
}

export function SettingsDialog({
  settings,
  theme,
  onThemeChange,
  onAllowSystemActionsChange,
  onRefreshIntervalChange,
  onPreferredEditorChange,
  onGroupByDirectoryChange,
  onShowChangeToastsChange,
  onWatchedPortNotificationsChange,
  onWatchedPortsChange,
  onIncludeUdpChange,
  onUseHttpsForLocalhostChange,
  onLiquidGlassChange,
  onGlassTranslucencyChange,
  onGlassBlurChange,
  onGlassTintChange,
  trigger,
}: SettingsDialogProps) {
  const [open, setOpen] = useState(false);
  const [watchedPortInput, setWatchedPortInput] = useState("");
  const [historyVersion, setHistoryVersion] = useState(0);
  const [selectedHistoryPort, setSelectedHistoryPort] = useState<number | null>(null);
  const [cliStatus, setCliStatus] = useState<CliInstallStatus | null>(null);
  const [cliBusy, setCliBusy] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }

    let cancelled = false;

    void fetchCliInstallStatus().then((status) => {
      if (!cancelled) {
        setCliStatus(status);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [open]);

  const refreshCliStatus = async () => {
    const status = await fetchCliInstallStatus();
    setCliStatus(status);
    return status;
  };

  const handleInstallCli = async () => {
    setCliBusy(true);
    try {
      await installCliToPath();
      await refreshCliStatus();
      toast.success("Command-line tool installed", {
        description: "Run port-watch check 3000 from Terminal.",
      });
    } catch (err) {
      toast.error("Could not install CLI", {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setCliBusy(false);
    }
  };

  const handleUninstallCli = async () => {
    setCliBusy(true);
    try {
      await uninstallCliFromPath();
      await refreshCliStatus();
      toast.success("Command-line tool removed");
    } catch (err) {
      toast.error("Could not uninstall CLI", {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setCliBusy(false);
    }
  };

  const addWatchedPort = () => {
    const port = Number.parseInt(watchedPortInput.trim(), 10);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      return;
    }
    if (settings.watchedPorts.includes(port)) {
      setWatchedPortInput("");
      return;
    }
    onWatchedPortsChange([...settings.watchedPorts, port].sort((a, b) => a - b));
    setWatchedPortInput("");
  };

  const removeWatchedPort = (port: number) => {
    onWatchedPortsChange(settings.watchedPorts.filter((item) => item !== port));
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent className="flex max-h-[min(85vh,720px)] w-[calc(100%-2rem)] max-w-lg flex-col gap-0 overflow-hidden p-0 sm:w-full">
        <DialogHeader className="shrink-0 border-b px-6 pt-6 pr-12 pb-4">
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Configure refresh, appearance, filters, and workflow preferences.
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto overflow-x-hidden px-6 py-5">
          <SettingSection title="Refresh">
            <SettingRow
              label="Auto-refresh"
              description="How often to scan listening ports."
            >
              <Select
                value={String(settings.refreshIntervalMs)}
                onValueChange={(v) =>
                  onRefreshIntervalChange(Number(v) as RefreshInterval)
                }
              >
                <SelectTrigger size="sm" className="w-full min-w-[8.5rem] sm:w-[8.5rem]">
                  <SelectValue placeholder="Auto-refresh" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="3000">Every 3s</SelectItem>
                  <SelectItem value="10000">Every 10s</SelectItem>
                  <SelectItem value="0">Off</SelectItem>
                </SelectContent>
              </Select>
            </SettingRow>
            <SettingRow
              htmlFor="settings-include-udp"
              label="Include UDP listeners"
              description="Scan UDP sockets in addition to TCP listeners."
            >
              <Switch
                id="settings-include-udp"
                checked={settings.includeUdp}
                onCheckedChange={onIncludeUdpChange}
              />
            </SettingRow>
          </SettingSection>

          <SettingSection title="Appearance">
            <SettingRow
              label="Theme"
              description="Light, grey dark, OLED dark, or match system."
            >
              <div
                className="flex items-center rounded-lg border bg-muted/40 p-0.5"
                role="group"
                aria-label="Theme"
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
            </SettingRow>
            {isMacOS() && (
              <SettingRow
                htmlFor="settings-liquid-glass"
                label="Liquid Glass"
                description="Native macOS translucency. Requires window transparency."
              >
                <Switch
                  id="settings-liquid-glass"
                  checked={settings.liquidGlass}
                  onCheckedChange={onLiquidGlassChange}
                />
              </SettingRow>
            )}
            {isMacOS() && settings.liquidGlass && (
              <>
                <SettingRow
                  label="Translucency"
                  description="How much shows through the window."
                >
                  <Select
                    value={settings.glassTranslucency}
                    onValueChange={(v) =>
                      onGlassTranslucencyChange(
                        v as AppSettings["glassTranslucency"],
                      )
                    }
                  >
                    <SelectTrigger size="sm" className="w-full min-w-[8.5rem] sm:w-[8.5rem]">
                      <SelectValue placeholder="Translucency" />
                    </SelectTrigger>
                    <SelectContent>
                      {GLASS_TRANSLUCENCY_OPTIONS.map(({ value, label }) => (
                        <SelectItem key={value} value={value}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </SettingRow>
                <SettingRow
                  label="Blur"
                  description="Frost strength behind the glass."
                >
                  <Select
                    value={settings.glassBlur}
                    onValueChange={(v) =>
                      onGlassBlurChange(v as AppSettings["glassBlur"])
                    }
                  >
                    <SelectTrigger size="sm" className="w-full min-w-[8.5rem] sm:w-[8.5rem]">
                      <SelectValue placeholder="Blur" />
                    </SelectTrigger>
                    <SelectContent>
                      {GLASS_BLUR_OPTIONS.map(({ value, label }) => (
                        <SelectItem key={value} value={value}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </SettingRow>
                <SettingRow
                  htmlFor="settings-glass-tint"
                  label="Tint dark themes"
                  description="Deepen the glass in grey and OLED themes for contrast."
                >
                  <Switch
                    id="settings-glass-tint"
                    checked={settings.glassTint}
                    onCheckedChange={onGlassTintChange}
                  />
                </SettingRow>
              </>
            )}
          </SettingSection>

          <SettingSection title="Table">
            <SettingRow
              htmlFor="settings-group-by-directory"
              label="Group by directory"
              description="Cluster processes from the same project."
            >
              <Switch
                id="settings-group-by-directory"
                checked={settings.groupByDirectory}
                onCheckedChange={onGroupByDirectoryChange}
              />
            </SettingRow>
            <SettingRow
              htmlFor="settings-allow-system-actions"
              label="Allow system process actions"
              description="Enable stop and delete on system services."
            >
              <Switch
                id="settings-allow-system-actions"
                checked={settings.allowSystemProcessActions}
                onCheckedChange={onAllowSystemActionsChange}
              />
            </SettingRow>
          </SettingSection>

          <SettingSection title="Notifications">
            <SettingRow
              htmlFor="settings-show-change-toasts"
              label="Port change toasts"
              description="In-app toasts when ports are taken or freed."
            >
              <Switch
                id="settings-show-change-toasts"
                checked={settings.showChangeToasts}
                onCheckedChange={onShowChangeToastsChange}
              />
            </SettingRow>
            <SettingRow
              htmlFor="settings-watched-port-notifications"
              label="Watched port alerts"
              description="Desktop notifications when watched ports change."
            >
              <Switch
                id="settings-watched-port-notifications"
                checked={settings.watchedPortNotifications}
                onCheckedChange={onWatchedPortNotificationsChange}
              />
            </SettingRow>
            <div className="py-3">
              <Label className="text-sm font-medium">Watched ports</Label>
              <p className="mb-2 text-xs text-muted-foreground">
                Get native alerts when these ports are occupied or freed.
              </p>
              <div className="flex gap-2">
                <Input
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
                <Button type="button" size="sm" variant="outline" onClick={addWatchedPort}>
                  Add
                </Button>
              </div>
              {settings.watchedPorts.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {settings.watchedPorts.map((port) => (
                    <Button
                      key={port}
                      type="button"
                      size="sm"
                      variant="secondary"
                      className="h-7 font-mono text-xs"
                      onClick={() => removeWatchedPort(port)}
                    >
                      {port} ×
                    </Button>
                  ))}
                </div>
              )}
            </div>
          </SettingSection>

          <SettingSection title="Port history">
            <div className="py-3">
              <div className="mb-2 flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium">Per-port timeline</p>
                  <p className="text-xs text-muted-foreground">
                    First and last seen for each port, grouped by today, yesterday,
                    and earlier.
                  </p>
                </div>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  className="shrink-0"
                  onClick={() => {
                    clearPortHistory();
                    setSelectedHistoryPort(null);
                    setHistoryVersion((value) => value + 1);
                  }}
                >
                  <Trash2Icon data-icon="inline-start" />
                  Clear
                </Button>
              </div>
              <div key={historyVersion} className="max-h-56 overflow-y-auto">
                <PortHistoryList
                  selectedPort={selectedHistoryPort}
                  onSelectPort={(port) =>
                    setSelectedHistoryPort((current) =>
                      current === port ? null : port,
                    )
                  }
                />
              </div>
            </div>
          </SettingSection>

          <SettingSection title="Workflow">
            <SettingRow
              htmlFor="settings-use-https-localhost"
              label="Use HTTPS for localhost URLs"
              description="Open in Browser and Copy URL use https://localhost instead of http://."
            >
              <Switch
                id="settings-use-https-localhost"
                checked={settings.useHttpsForLocalhost}
                onCheckedChange={onUseHttpsForLocalhostChange}
              />
            </SettingRow>
            <SettingRow
              stacked
              label="Open in editor"
              description="Default editor for the row action."
            >
              <Select
                value={settings.preferredEditor}
                onValueChange={(v) =>
                  onPreferredEditorChange(v as AppSettings["preferredEditor"])
                }
              >
                <SelectTrigger size="sm" className="w-full">
                  <SelectValue placeholder="Editor" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="cursor">Cursor</SelectItem>
                  <SelectItem value="code">VS Code</SelectItem>
                </SelectContent>
              </Select>
            </SettingRow>
          </SettingSection>

          <SettingSection title="Command line">
            <div className="py-3">
              <div className="mb-3 flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium">Install CLI to PATH</p>
                  <p className="text-xs text-muted-foreground">
                    Run{" "}
                    <span className="font-mono">port-watch check 3000</span> from
                    Terminal and CI scripts.
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {cliStatus?.pointsToApp
                      ? `Installed at ${cliStatus.linkPath}`
                      : cliStatus?.installed
                        ? `Another port-watch is installed at ${cliStatus.linkPath}`
                        : "Not installed"}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {cliInstallPrivilegeHint()}
                  </p>
                </div>
                <div className="flex shrink-0 flex-wrap gap-2">
                  {!cliStatus?.pointsToApp && (
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={cliBusy}
                      onClick={() => void handleInstallCli()}
                    >
                      <TerminalIcon data-icon="inline-start" />
                      {cliBusy ? "Working…" : "Install"}
                    </Button>
                  )}
                  {cliStatus?.pointsToApp && (
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={cliBusy}
                      onClick={() => void handleUninstallCli()}
                    >
                      {cliBusy ? "Working…" : "Uninstall"}
                    </Button>
                  )}
                </div>
              </div>
            </div>
          </SettingSection>

          {isMacOS() && (
            <SettingSection title="Menu bar">
              <SettingRow
                label="Menu bar mode"
                description="Toggle this from the menu bar icon's menu. Hides the dock icon so Port Watch runs only in the menu bar; clicking the icon opens a native menu of your active ports."
              >
                <span className="text-xs font-medium text-muted-foreground">
                  {settings.menuBarMode ? "On" : "Off"}
                </span>
              </SettingRow>
            </SettingSection>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
