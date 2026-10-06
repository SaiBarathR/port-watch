import { useCallback, useEffect, useRef } from "react";
import {
  BracesIcon,
  EllipsisIcon,
  FileTextIcon,
  HistoryIcon,
  OctagonIcon,
  RefreshCwIcon,
  SearchIcon,
  SettingsIcon,
  XIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { NotificationMenu } from "@/components/notification-menu";
import { SegmentedControl } from "@/components/ui/segmented-control";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProcessActions } from "@/components/process-actions";
import { SettingsDialog } from "@/components/settings-dialog";
import type { ThemeMode } from "@/hooks/use-theme";
import { processesToJson, processesToMarkdown } from "@/lib/export-snapshot";
import { isMacOS } from "@/lib/platform";
import { focusTabStopRow } from "@/lib/row-focus";
import { formatHistorySeen, getPortSummary } from "@/lib/port-history";
import {
  listenerScope,
  setListenerScope,
  type ListenerScope,
} from "@/lib/settings-actions";
import { updateSettings } from "@/lib/settings-store";
import {
  SEARCH_FIELD_OPTIONS,
  userProcesses,
  type AppSettings,
  type PortProcess,
  type SearchField,
} from "@/lib/types";

interface PortToolbarProps {
  search: string;
  onSearchChange: (value: string) => void;
  portLookupEmpty: boolean;
  exactPortQuery: number | null;
  portLookupOccupants: PortProcess[];
  /** The rows in view: what an export copies and "stop all" stops. */
  shownProcesses: PortProcess[];
  settings: AppSettings;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  onRefresh: () => void;
  loading: boolean;
  /** True until the first scan has come back. */
  firstScanPending: boolean;
  userCount: number;
  systemCount: number;
}

function Count({ children }: { children: number }) {
  return (
    <span className="text-xs font-normal text-muted-foreground tabular-nums">
      {children}
    </span>
  );
}

export function PortToolbar({
  search,
  onSearchChange,
  portLookupEmpty,
  exactPortQuery,
  portLookupOccupants,
  shownProcesses,
  settings,
  theme,
  onThemeChange,
  onRefresh,
  loading,
  firstScanPending,
  userCount,
  systemCount,
}: PortToolbarProps) {
  const { canStop, freePort, stop, showHistory } = useProcessActions();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchField = settings.searchField;
  const selectedField =
    SEARCH_FIELD_OPTIONS.find((option) => option.value === searchField) ??
    SEARCH_FIELD_OPTIONS[0];

  const clearSearch = useCallback(() => {
    onSearchChange("");
    searchInputRef.current?.focus();
  }, [onSearchChange]);

  const handleSearchFieldChange = useCallback(
    (field: SearchField) => {
      if ((field === "port" || field === "pid") && /\D/.test(search)) {
        onSearchChange("");
      }
      updateSettings({ searchField: field });
    },
    [onSearchChange, search],
  );

  // ⌘F and "/" go to the search box, and ⌘R refreshes. ⌘K used to be the
  // search key and still is, unless a row has the keys: there it opens the
  // row's actions, and the table keeps the event to itself.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const mod = isMacOS() ? event.metaKey : event.ctrlKey;
      const key = event.key.toLowerCase();
      const typing =
        event.target instanceof HTMLElement &&
        (event.target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(event.target.tagName));

      if ((mod && (key === "f" || key === "k")) || (key === "/" && !typing)) {
        event.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      } else if (mod && key === "r") {
        event.preventDefault();
        onRefresh();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onRefresh]);

  const copyExport = async (format: "json" | "markdown") => {
    const text =
      format === "json"
        ? processesToJson(shownProcesses)
        : processesToMarkdown(shownProcesses);
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

  const shownUserProcesses = userProcesses(shownProcesses).filter(canStop);

  return (
    <div className="flex flex-col gap-3 border-b pb-3">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex min-w-[280px] flex-1 items-stretch overflow-hidden rounded-xl border bg-muted/20 shadow-xs transition-[box-shadow,border-color] focus-within:border-ring/60 focus-within:ring-2 focus-within:ring-ring/30">
          <Select
            value={searchField}
            onValueChange={(value) =>
              handleSearchFieldChange(value as SearchField)
            }
          >
            <SelectTrigger
              aria-label="Search in"
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
              aria-label="Search listeners"
              className="h-9 w-full rounded-none border-0 bg-transparent py-0 pr-16 pl-9 shadow-none focus-visible:ring-0"
              placeholder={searchPlaceholder}
              inputMode={
                searchField === "port" || searchField === "pid"
                  ? "numeric"
                  : "search"
              }
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
                } else if (e.key === "ArrowDown") {
                  // Down from the search box is into the results.
                  e.preventDefault();
                  focusTabStopRow();
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
                {isMacOS() ? "⌘F" : "Ctrl F"}
              </kbd>
            </div>
          </div>
        </div>

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

        <NotificationMenu settings={settings} />

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="outline"
              size="icon"
              className="size-9 shrink-0"
              aria-label="More actions"
            >
              <EllipsisIcon />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64">
            <DropdownMenuLabel>Export the rows shown</DropdownMenuLabel>
            <DropdownMenuItem onClick={() => void copyExport("json")}>
              <BracesIcon />
              Copy as JSON
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => void copyExport("markdown")}>
              <FileTextIcon />
              Copy as Markdown
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              disabled={shownUserProcesses.length === 0}
              onClick={() =>
                stop(
                  shownUserProcesses,
                  `Stop all ${shownUserProcesses.length} user processes shown?`,
                  "This stops every user process in the table as it is filtered now. System services are left alone.",
                )
              }
            >
              <OctagonIcon />
              Stop all user processes shown…
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <SettingsDialog
          settings={settings}
          theme={theme}
          onThemeChange={onThemeChange}
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

      {/* One line of fixed height: a port lookup answers here instead of
          pushing the table down while the port is still being typed. */}
      <div className="flex h-8 items-center gap-3">
        <SegmentedControl<ListenerScope>
          aria-label="Listeners shown"
          value={listenerScope(settings)}
          onValueChange={setListenerScope}
          options={[
            {
              value: "user",
              label: (
                <>
                  User <Count>{userCount}</Count>
                </>
              ),
            },
            {
              value: "system",
              label: (
                <>
                  System <Count>{systemCount}</Count>
                </>
              ),
            },
            {
              value: "all",
              label: (
                <>
                  All <Count>{userCount + systemCount}</Count>
                </>
              ),
            },
          ]}
        />

        <div
          role="status"
          className="flex min-w-0 flex-1 items-center justify-end gap-2 text-sm text-muted-foreground"
        >
          {exactPortQuery !== null && portLookupOccupants.length > 0 ? (
            <>
              <span className="truncate">
                <span className="font-medium text-foreground">
                  Port {exactPortQuery}
                </span>{" "}
                is in use by{" "}
                {portLookupOccupants
                  .map((process) => `${process.name} (PID ${process.pid})`)
                  .join(", ")}
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 shrink-0"
                onClick={() => showHistory(exactPortQuery)}
              >
                <HistoryIcon />
                History
              </Button>
              <Button
                variant="destructive"
                size="sm"
                className="h-7 shrink-0"
                disabled={!portLookupOccupants.some(canStop)}
                onClick={() => freePort(exactPortQuery)}
              >
                <OctagonIcon />
                Free port {exactPortQuery}
              </Button>
            </>
          ) : portLookupEmpty && exactPortQuery !== null ? (
            <>
              <span className="truncate">
                <span className="font-medium text-foreground">
                  Port {exactPortQuery}
                </span>{" "}
                is free
                <LastHolder port={exactPortQuery} />
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 shrink-0"
                onClick={() => showHistory(exactPortQuery)}
              >
                <HistoryIcon />
                History
              </Button>
            </>
          ) : firstScanPending ? (
            "Scanning ports…"
          ) : settings.includeUdp ? (
            "TCP and UDP"
          ) : null}
        </div>
      </div>
    </div>
  );
}

// Who last held a free port, from the history kept on this machine.
function LastHolder({ port }: { port: number }) {
  const summary = getPortSummary(port);
  if (!summary) {
    return null;
  }
  return (
    <>
      {" "}
      · last held by {summary.lastProcessName},{" "}
      {formatHistorySeen(summary.lastSeen)}
    </>
  );
}
