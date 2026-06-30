import { useCallback, useEffect, useRef } from "react";
import {
  BracesIcon,
  FileTextIcon,
  FilterIcon,
  OctagonIcon,
  RefreshCwIcon,
  SearchIcon,
  SettingsIcon,
  ShareIcon,
  XIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { PortHistoryTimeline } from "@/components/port-history-timeline";
import { SettingsDialog } from "@/components/settings-dialog";
import type { ThemeMode } from "@/hooks/use-theme";
import { processesToJson, processesToMarkdown } from "@/lib/export-snapshot";
import {
  SEARCH_FIELD_OPTIONS,
  type AppSettings,
  type PortProcess,
  type RefreshInterval,
  type SearchField,
} from "@/lib/types";

interface PortToolbarProps {
  search: string;
  onSearchChange: (value: string) => void;
  searchField: SearchField;
  onSearchFieldChange: (field: SearchField) => void;
  portLookupEmpty: boolean;
  exactPortQuery: number | null;
  portLookupOccupants: PortProcess[];
  exportProcesses: PortProcess[];
  settings: AppSettings;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  onHideSystemChange: (hide: boolean) => void;
  onHideUserChange: (hide: boolean) => void;
  onAllowSystemActionsChange: (allow: boolean) => void;
  onRefreshIntervalChange: (interval: RefreshInterval) => void;
  onPreferredEditorChange: (editor: AppSettings["preferredEditor"]) => void;
  onGroupByDirectoryChange: (group: boolean) => void;
  onShowChangeToastsChange: (show: boolean) => void;
  onWatchedPortNotificationsChange: (enabled: boolean) => void;
  onWatchedPortsChange: (ports: number[]) => void;
  onIncludeUdpChange: (include: boolean) => void;
  onUseHttpsForLocalhostChange: (useHttps: boolean) => void;
  onFreePort: (port: number, occupants: PortProcess[]) => void;
  onRefresh: () => void;
  loading: boolean;
  userCount: number;
  systemCount: number;
  hiddenSystemCount: number;
  hiddenUserCount: number;
}

export function PortToolbar({
  search,
  onSearchChange,
  searchField,
  onSearchFieldChange,
  portLookupEmpty,
  exactPortQuery,
  portLookupOccupants,
  exportProcesses,
  settings,
  theme,
  onThemeChange,
  onHideSystemChange,
  onHideUserChange,
  onAllowSystemActionsChange,
  onRefreshIntervalChange,
  onPreferredEditorChange,
  onGroupByDirectoryChange,
  onShowChangeToastsChange,
  onWatchedPortNotificationsChange,
  onWatchedPortsChange,
  onIncludeUdpChange,
  onUseHttpsForLocalhostChange,
  onFreePort,
  onRefresh,
  loading,
  userCount,
  systemCount,
  hiddenSystemCount,
  hiddenUserCount,
}: PortToolbarProps) {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const selectedField =
    SEARCH_FIELD_OPTIONS.find((option) => option.value === searchField) ??
    SEARCH_FIELD_OPTIONS[0];

  const clearSearch = useCallback(() => {
    onSearchChange("");
    searchInputRef.current?.focus();
  }, [onSearchChange]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        onSearchFieldChange("port");
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onSearchFieldChange]);

  const copyExport = async (format: "json" | "markdown") => {
    const text =
      format === "json"
        ? processesToJson(exportProcesses)
        : processesToMarkdown(exportProcesses);
    try {
      await navigator.clipboard.writeText(text);
      toast.success(
        format === "json" ? "Copied JSON snapshot" : "Copied Markdown snapshot",
      );
    } catch (err) {
      toast.error(String(err));
    }
  };

  const searchPlaceholder =
    searchField === "port"
      ? "Search by port number…"
      : searchField === "all"
        ? "Search ports, processes, paths, PID…"
        : `Search by ${selectedField.label.toLowerCase()}…`;

  const listenerSummary = [
    `${userCount} user listener${userCount === 1 ? "" : "s"}`,
    hiddenUserCount > 0 ? `${hiddenUserCount} user (hidden)` : null,
    hiddenSystemCount > 0 ? `${hiddenSystemCount} system (hidden)` : null,
    !settings.hideSystemServices && systemCount > 0
      ? `${systemCount} system`
      : null,
    settings.includeUdp ? "UDP included" : null,
  ]
    .filter(Boolean)
    .join(" · ");

  const stoppableOccupants = portLookupOccupants.filter(
    (process) =>
      !process.is_system_service || settings.allowSystemProcessActions,
  );

  return (
    <div className="flex flex-col gap-3 border-b pb-4">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex min-w-[280px] flex-1 items-stretch overflow-hidden rounded-xl border bg-muted/20 shadow-xs transition-[box-shadow,border-color] focus-within:border-ring/60 focus-within:ring-2 focus-within:ring-ring/30">
          <Select
            value={searchField}
            onValueChange={(value) => onSearchFieldChange(value as SearchField)}
          >
            <SelectTrigger
              className="h-9 w-[118px] shrink-0 self-stretch rounded-none border-0 bg-transparent py-0 shadow-none focus-visible:ring-0"
            >
              <SelectValue placeholder="Field" />
            </SelectTrigger>
            <SelectContent align="start">
              {SEARCH_FIELD_OPTIONS.map(({ value, label }) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <div className="w-px shrink-0 self-stretch bg-border" aria-hidden />

          <div className="relative flex min-w-0 flex-1 items-stretch">
            <SearchIcon
              className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <Input
              ref={searchInputRef}
              className="h-9 w-full rounded-none border-0 bg-transparent py-0 pl-9 pr-16 shadow-none focus-visible:ring-0"
              placeholder={searchPlaceholder}
              inputMode={searchField === "port" || searchField === "pid" ? "numeric" : "search"}
              value={search}
              onChange={(e) => {
                const value = e.target.value;
                if (searchField === "port" || searchField === "pid") {
                  onSearchChange(value.replace(/\D/g, ""));
                  return;
                }
                onSearchChange(value);
              }}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  clearSearch();
                }
              }}
            />
            <div className="absolute top-1/2 right-2 flex -translate-y-1/2 items-center gap-1">
              {search && (
                <button
                  type="button"
                  className="rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                  onClick={clearSearch}
                  aria-label="Clear search"
                >
                  <XIcon className="size-3.5" />
                </button>
              )}
              <kbd className="hidden rounded border bg-background/80 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground sm:inline">
                ⌘K
              </kbd>
            </div>
          </div>
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="outline"
              size="icon"
              className="size-9 shrink-0"
              aria-label="Filter listeners"
            >
              <FilterIcon />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-52">
            <DropdownMenuLabel>Show listeners</DropdownMenuLabel>
            <DropdownMenuCheckboxItem
              checked={!settings.hideUserServices}
              onCheckedChange={(checked) => onHideUserChange(!checked)}
              onSelect={(event) => event.preventDefault()}
            >
              User services
            </DropdownMenuCheckboxItem>
            <DropdownMenuCheckboxItem
              checked={!settings.hideSystemServices}
              onCheckedChange={(checked) => onHideSystemChange(!checked)}
              onSelect={(event) => event.preventDefault()}
            >
              System services
            </DropdownMenuCheckboxItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="outline"
              size="icon"
              className="size-9 shrink-0"
              aria-label="Export snapshot"
            >
              <ShareIcon />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-52">
            <DropdownMenuLabel>Export filtered view</DropdownMenuLabel>
            <DropdownMenuItem onClick={() => void copyExport("json")}>
              <BracesIcon data-icon="inline-start" />
              Copy as JSON
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => void copyExport("markdown")}>
              <FileTextIcon data-icon="inline-start" />
              Copy as Markdown
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <Button
          variant="outline"
          size="icon"
          className="size-9 shrink-0"
          onClick={onRefresh}
          disabled={loading}
          aria-label="Refresh"
        >
          <RefreshCwIcon className={loading ? "animate-spin" : ""} />
        </Button>

        <SettingsDialog
          settings={settings}
          theme={theme}
          onThemeChange={onThemeChange}
          onAllowSystemActionsChange={onAllowSystemActionsChange}
          onRefreshIntervalChange={onRefreshIntervalChange}
          onPreferredEditorChange={onPreferredEditorChange}
          onGroupByDirectoryChange={onGroupByDirectoryChange}
          onShowChangeToastsChange={onShowChangeToastsChange}
          onWatchedPortNotificationsChange={onWatchedPortNotificationsChange}
          onWatchedPortsChange={onWatchedPortsChange}
          onIncludeUdpChange={onIncludeUdpChange}
          onUseHttpsForLocalhostChange={onUseHttpsForLocalhostChange}
          trigger={
            <Button
              variant="outline"
              size="icon"
              className="size-9 shrink-0"
              aria-label="Settings"
            >
              <SettingsIcon />
            </Button>
          }
        />
      </div>

      <p className="text-sm text-muted-foreground">{listenerSummary}</p>

      {portLookupEmpty && exactPortQuery !== null && (
        <Alert>
          <AlertTitle>Nothing listening on port {exactPortQuery}</AlertTitle>
          <AlertDescription className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <span>No process is bound to this port right now.</span>
              <Button variant="outline" size="sm" onClick={onRefresh} disabled={loading}>
                Check again
              </Button>
            </div>
            <PortHistoryTimeline port={exactPortQuery} />
          </AlertDescription>
        </Alert>
      )}

      {exactPortQuery !== null && portLookupOccupants.length > 0 && (
        <Alert>
          <AlertTitle>
            Port {exactPortQuery} in use by {portLookupOccupants.length} process
            {portLookupOccupants.length === 1 ? "" : "es"}
          </AlertTitle>
          <AlertDescription className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <span>
                {portLookupOccupants
                  .map((process) => `${process.name} (PID ${process.pid})`)
                  .join(", ")}
              </span>
              <Button
                variant="destructive"
                size="sm"
                disabled={stoppableOccupants.length === 0}
                onClick={() => onFreePort(exactPortQuery, stoppableOccupants)}
              >
                <OctagonIcon data-icon="inline-start" />
                Free port {exactPortQuery}
              </Button>
            </div>
            <PortHistoryTimeline port={exactPortQuery} />
          </AlertDescription>
        </Alert>
      )}

    </div>
  );
}
