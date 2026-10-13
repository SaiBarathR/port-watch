import type { ComponentType, ReactNode } from "react";
import {
  BellIcon,
  BellOffIcon,
  BracesIcon,
  BookOpenIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  CodeIcon,
  CopyIcon,
  EllipsisIcon,
  ExternalLinkIcon,
  EyeIcon,
  FileTextIcon,
  FolderOpenIcon,
  GlobeIcon,
  HistoryIcon,
  InfoIcon,
  KeyboardIcon,
  LinkIcon,
  ListXIcon,
  LockIcon,
  OctagonIcon,
  OctagonXIcon,
  PinIcon,
  RefreshCwIcon,
  SearchIcon,
  SettingsIcon,
  SquareTerminalIcon,
  TerminalIcon,
  TrashIcon,
  type LucideIcon,
} from "lucide-react";
import { stopProcessDescription } from "@/lib/platform";
import {
  chordText,
  editorName,
  revealLabel,
  trashName,
  welcomeKeys,
  type WelcomeContext,
  type WelcomePage,
  type WelcomeTint,
} from "@/lib/welcome-pages";
import type { WelcomePageId } from "@/lib/welcome";
import { cn } from "@/lib/utils";
import appIcon from "../../src-tauri/icons/128x128@2x.png";

// The picture at the head of a page of the guide: the part of the window the
// page is about, drawn small, with a numbered dot on each thing the cards
// under it go on to explain.
//
// Drawn, not photographed. A screenshot is a file to ship and to keep up
// with the app, shows one theme and one platform's keys, and is somebody's
// machine.

/** The colours of a page: its symbol in the list, and behind its picture. */
export const WELCOME_TINTS: Record<WelcomeTint, { tile: string; art: string }> =
  {
    teal: {
      tile: "from-teal-400 to-teal-600",
      art: "from-teal-500/30 to-teal-500/10",
    },
    sky: {
      tile: "from-sky-400 to-sky-600",
      art: "from-sky-500/30 to-sky-500/10",
    },
    violet: {
      tile: "from-violet-400 to-violet-600",
      art: "from-violet-500/30 to-violet-500/10",
    },
    rose: {
      tile: "from-rose-400 to-rose-600",
      art: "from-rose-500/30 to-rose-500/10",
    },
    amber: {
      tile: "from-amber-400 to-amber-600",
      art: "from-amber-500/30 to-amber-500/10",
    },
    indigo: {
      tile: "from-indigo-400 to-indigo-600",
      art: "from-indigo-500/30 to-indigo-500/10",
    },
    zinc: {
      tile: "from-zinc-400 to-zinc-600",
      art: "from-zinc-500/30 to-zinc-500/10",
    },
  };

/**
 * A numbered dot: on a picture, and on the card that says what it is
 * pointing at.
 */
export function GuideBadge({
  number,
  className,
  onPicture = false,
}: {
  number: number;
  className?: string;
  /** On a picture, where the test that holds it to its cards counts it. */
  onPicture?: boolean;
}) {
  return (
    <span
      aria-hidden
      data-guide-mark={onPicture ? number : undefined}
      className={cn(
        "inline-flex size-[17px] shrink-0 items-center justify-center rounded-full bg-sky-600 font-sans text-[10px] leading-none font-bold text-white tabular-nums shadow-sm ring-[1.5px] ring-background",
        className,
      )}
    >
      {number}
    </span>
  );
}

/** A page's picture is given this to ask which number a tip was dealt. */
type Numbers = (mark: string) => number | undefined;

interface PictureProps {
  n: Numbers;
  context: WelcomeContext;
}

const PIN = {
  top: "-top-2.5 left-1/2 -translate-x-1/2",
  "top-right": "-top-2.5 -right-3",
  "top-left": "-top-2.5 -left-3",
  left: "top-1/2 -left-5 -translate-y-1/2",
  right: "top-1/2 -right-5 -translate-y-1/2",
  // Beside it, for something too small to have a dot on top of it.
  after: "top-1/2 left-full ml-1 -translate-y-1/2",
  // Astride the edge, for something that runs the width of its box.
  "left-edge": "top-1/2 -left-3.5 -translate-y-1/2",
} as const;

/**
 * A tip's dot, pinned to an edge or a corner of the box it is in. Nothing
 * where the page has no such tip.
 */
function Pin({
  number,
  at,
}: {
  number: number | undefined;
  at: keyof typeof PIN;
}) {
  if (number === undefined) {
    return null;
  }
  return (
    <GuideBadge
      number={number}
      onPicture
      className={cn("absolute z-10", PIN[at])}
    />
  );
}

/** Something in a picture, with a tip's dot pinned to it. */
function Marked({
  number,
  at = "top-right",
  className,
  children,
}: {
  number: number | undefined;
  at?: keyof typeof PIN;
  className?: string;
  children: ReactNode;
}) {
  return (
    <span className={cn("relative inline-flex", className)}>
      {children}
      <Pin number={number} at={at} />
    </span>
  );
}

// ---------------------------------------------------------------------------
// Parts the pictures share
// ---------------------------------------------------------------------------

/** A push button, as the app draws them. */
function ArtButton({
  variant = "outline",
  className,
  children,
}: {
  variant?: "outline" | "primary" | "destructive" | "ghost";
  className?: string;
  children: ReactNode;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-[18px] shrink-0 items-center gap-1 rounded-[5px] px-1.5 font-medium whitespace-nowrap [&_svg]:size-2.5",
        variant === "outline" && "border bg-background",
        variant === "primary" && "bg-primary text-primary-foreground",
        variant === "destructive" && "bg-destructive text-white",
        className,
      )}
    >
      {children}
    </span>
  );
}

/** One of the square buttons at the end of the toolbar. */
function ArtToolButton({
  icon: Icon,
  active = false,
}: {
  icon: LucideIcon;
  active?: boolean;
}) {
  return (
    <span
      className={cn(
        "inline-flex size-5 shrink-0 items-center justify-center rounded-[5px] border bg-background",
        active && "bg-accent",
      )}
    >
      <Icon className="size-2.5" />
    </span>
  );
}

/** The box at the head of a row. */
function ArtCheckbox({
  checked = false,
  disabled = false,
}: {
  checked?: boolean;
  disabled?: boolean;
}) {
  return (
    <span
      className={cn(
        "inline-flex size-2.5 shrink-0 items-center justify-center rounded-[2px] border border-foreground/40",
        checked && "border-primary bg-primary",
        disabled && "border-foreground/15 bg-muted",
      )}
    >
      {checked && (
        <svg
          viewBox="0 0 10 10"
          className="size-2 fill-none stroke-primary-foreground stroke-2"
        >
          <path d="M2 5.2 4.2 7.4 8 3" />
        </svg>
      )}
    </span>
  );
}

/** The User / System / All switch over the table. */
function ArtScope() {
  return (
    <span className="inline-flex shrink-0 items-center rounded-md border bg-muted/40 p-px">
      <span className="rounded-[4px] bg-background px-1.5 py-0.5 font-medium shadow-xs">
        User <span className="font-normal text-muted-foreground">3</span>
      </span>
      <span className="px-1.5 py-0.5 text-muted-foreground">System 12</span>
      <span className="px-1.5 py-0.5 text-muted-foreground">All 15</span>
    </span>
  );
}

function ArtTable({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col rounded-md border [&>*:last-child]:border-b-0">
      {children}
    </div>
  );
}

/** A heading between rows: "Pinned", or a folder when grouping. */
function ArtGroup({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-1.5 border-b bg-muted/30 px-2 py-0.5 text-[9px] font-medium text-muted-foreground first:rounded-t-[5px]">
      {children}
    </div>
  );
}

interface ArtRowProps {
  port: number;
  /** What the port number alone suggests: "Vite", "Dev server". */
  hint?: string;
  /** Open to other machines, where most are open to this one only. */
  open?: boolean;
  name: string;
  /** The project folder's own name, and where it sits. */
  folder?: [name: string, parent: string];
  uptime: string;
  /** The tag in the Type column, for a row that is the system's. */
  kind?: string;
  checked?: boolean;
  /** Not the user's to stop: its box cannot be ticked. */
  locked?: boolean;
  isNew?: boolean;
  /** With its "…" pressed. */
  menuOpen?: boolean;
  /** Without the uptime, for a picture with a menu beside the table. */
  compact?: boolean;
  marks?: {
    row?: number;
    reach?: number;
    change?: number;
    menu?: number;
  };
}

/** A row of the table, with the columns a window of this width has room for. */
function ArtRow({
  port,
  hint,
  open = false,
  name,
  folder,
  uptime,
  kind,
  checked = false,
  locked = false,
  isNew = false,
  menuOpen = false,
  compact = false,
  marks = {},
}: ArtRowProps) {
  const Reach = open ? GlobeIcon : LockIcon;
  return (
    <div
      className={cn(
        "relative grid items-center gap-x-2 border-b px-2 py-[3px]",
        compact
          ? "grid-cols-[10px_4rem_4.5rem_minmax(0,1fr)_18px]"
          : "grid-cols-[10px_6.5rem_4.25rem_minmax(0,1fr)_auto_18px]",
        isNew &&
          "bg-[color-mix(in_oklab,var(--color-emerald-500)_10%,var(--background))]",
        checked && "bg-accent",
      )}
    >
      <Pin number={marks.row} at="left-edge" />
      <ArtCheckbox checked={checked} disabled={locked} />
      <span className="flex min-w-0 flex-col">
        <span className="flex items-center gap-1">
          <span className="font-mono text-[11px] font-semibold tabular-nums">
            {port}
          </span>
          <Marked number={marks.reach} at="after">
            <Reach
              className={cn(
                "size-2.5",
                open
                  ? "text-amber-600 dark:text-amber-400"
                  : "text-muted-foreground/60",
              )}
            />
          </Marked>
          {isNew && (
            <Marked number={marks.change} at="after" className="ml-0.5">
              <span className="rounded-full border border-emerald-500/40 px-1 text-[7px] leading-[11px] font-medium text-emerald-600 uppercase dark:text-emerald-400">
                new
              </span>
            </Marked>
          )}
        </span>
        {hint && (
          <span className="text-[8px] leading-[10px] text-muted-foreground">
            {hint}
          </span>
        )}
      </span>
      <span className="truncate font-medium">{name}</span>
      <span className="flex min-w-0 items-baseline gap-1">
        {folder ? (
          <>
            <span className="shrink-0 font-medium">{folder[0]}</span>
            <span className="truncate text-[9px] text-muted-foreground">
              {folder[1]}
            </span>
          </>
        ) : kind ? (
          // The Type column, which a table with system rows in it has.
          <span className="rounded-full bg-muted px-1.5 text-[8px] leading-[13px] font-medium whitespace-nowrap text-muted-foreground">
            {kind}
          </span>
        ) : (
          <span className="text-muted-foreground">—</span>
        )}
      </span>
      {!compact && (
        <span className="font-mono text-[9px] text-muted-foreground tabular-nums">
          {uptime}
        </span>
      )}
      <Marked number={marks.menu} className="justify-self-end">
        <span
          className={cn(
            "inline-flex size-[18px] items-center justify-center rounded-[5px]",
            menuOpen && "bg-accent",
          )}
        >
          <EllipsisIcon className="size-2.5" />
        </span>
      </Marked>
    </div>
  );
}

/** A menu that is open. */
function ArtMenu({
  className,
  children,
}: {
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "flex flex-col rounded-md border bg-popover p-[3px] text-popover-foreground shadow-md",
        className,
      )}
    >
      {children}
    </div>
  );
}

/** The line at the head of a menu that says what the menu is about. */
function ArtMenuHeading({ children }: { children: ReactNode }) {
  return (
    <span className="truncate px-1.5 py-px text-[9px] text-muted-foreground">
      {children}
    </span>
  );
}

function ArtSeparator() {
  return <span className="-mx-[3px] my-[3px] h-px shrink-0 bg-border" />;
}

/** Where a picture leaves out part of a menu: the items are still there. */
function ArtOmitted() {
  return (
    <span className="-mx-[3px] my-[5px] shrink-0 border-t border-dashed border-foreground/30" />
  );
}

function ArtMenuItem({
  icon: Icon,
  label,
  shortcut,
  destructive = false,
  disabled = false,
  reason,
  submenu = false,
  mark,
  markAt = "right",
}: {
  icon?: LucideIcon;
  label: string;
  shortcut?: string;
  destructive?: boolean;
  disabled?: boolean;
  /** Why the item cannot be used, said under it. */
  reason?: string;
  submenu?: boolean;
  mark?: number;
  markAt?: "left" | "right";
}) {
  return (
    <span className="relative flex flex-col px-1.5 py-px">
      <span
        className={cn(
          "flex items-center gap-1.5",
          destructive && "text-destructive",
          disabled && "opacity-50",
        )}
      >
        {Icon && <Icon className="size-2.5 shrink-0" />}
        <span className="min-w-0 flex-1 truncate">{label}</span>
        {shortcut && (
          <span className="shrink-0 text-[9px] text-muted-foreground">
            {shortcut}
          </span>
        )}
        {submenu && (
          <ChevronRightIcon className="size-2.5 shrink-0 text-muted-foreground" />
        )}
      </span>
      {reason && (
        <span className="pl-4 text-[9px] leading-[11px] text-muted-foreground">
          {reason}
        </span>
      )}
      <Pin number={mark} at={markAt} />
    </span>
  );
}

// ---------------------------------------------------------------------------
// The pictures
// ---------------------------------------------------------------------------

function ListArt({ n }: PictureProps) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <Marked number={n("scope")}>
          <ArtScope />
        </Marked>
        <Marked number={n("live")} at="left">
          <span className="flex items-center gap-1 text-muted-foreground">
            <span className="size-1.5 rounded-full bg-emerald-500" />
            Live · every 3 s
          </span>
        </Marked>
      </div>
      <ArtTable>
        <ArtGroup>
          <Marked number={n("pinned")} at="right">
            Pinned
          </Marked>
        </ArtGroup>
        <ArtRow
          port={3000}
          hint="Dev server"
          name="node"
          folder={["dashboard", "~/Projects"]}
          uptime="1h 30m"
          marks={{ row: n("row") }}
        />
        <ArtGroup>Other listeners</ArtGroup>
        <ArtRow
          port={4567}
          open
          name="ruby"
          folder={["helpdesk", "~/Projects"]}
          uptime="4h 0m"
          marks={{ reach: n("reach") }}
        />
        <ArtRow
          port={5173}
          hint="Vite"
          isNew
          name="node"
          folder={["storefront", "~/Projects"]}
          uptime="4s"
          marks={{ change: n("change") }}
        />
      </ArtTable>
    </div>
  );
}

function FindArt({ n, context }: PictureProps) {
  const keys = welcomeKeys(context.platform);
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-stretch rounded-lg border bg-muted/20">
        <Marked number={n("field")} at="top-left">
          <span className="flex items-center gap-3 py-1 pr-2 pl-2.5">
            Port
            <ChevronDownIcon className="size-2.5 opacity-50" />
          </span>
        </Marked>
        <span className="w-px bg-border" />
        <Marked number={n("search")} at="top" className="flex-1">
          <span className="flex flex-1 items-center gap-1.5 px-2">
            <SearchIcon className="size-2.5 text-muted-foreground" />
            3000
            <span className="-ml-1 h-3 w-px bg-foreground" />
          </span>
        </Marked>
        <span className="mr-1.5 self-center rounded border bg-background px-1 font-mono text-[8px] leading-[12px] text-muted-foreground">
          {context.platform === "macos"
            ? chordText(keys.find, "macos")
            : keys.find.join(" ")}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <ArtScope />
        <span className="flex min-w-0 flex-1 items-center justify-end gap-1.5">
          <Marked number={n("answer")} at="left" className="min-w-0">
            <span className="truncate text-muted-foreground">
              <span className="font-medium text-foreground">Port 3000</span> is
              in use by node (PID 48211)
            </span>
          </Marked>
          <ArtButton variant="ghost">
            <HistoryIcon />
            History
          </ArtButton>
          <Marked number={n("free")}>
            <ArtButton variant="destructive">
              <OctagonIcon />
              Free port 3000
            </ArtButton>
          </Marked>
        </span>
      </div>
      <ArtTable>
        <ArtRow
          port={3000}
          hint="Dev server"
          name="node"
          folder={["dashboard", "~/Projects"]}
          uptime="1h 30m"
          marks={{ row: n("rows") }}
        />
      </ArtTable>
    </div>
  );
}

// Where a dev server can be run from that is not in anyone's home folder:
// the app will stop it there, and will not remove the folder.
const OUTSIDE_HOME = {
  macos: "/private/tmp",
  windows: "C:\\Temp",
  linux: "/tmp",
  unknown: "/tmp",
} as const;

function ActArt({ n, context }: PictureProps) {
  const { platform } = context;
  const keys = welcomeKeys(platform);
  const say = (chord: string[]) => chordText(chord, platform);
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_12.5rem] gap-x-5">
      <div className="flex min-w-0 flex-col justify-between gap-3">
        <ArtTable>
          <ArtRow
            compact
            port={3000}
            hint="Dev server"
            name="node"
            folder={["dashboard", "~/Projects"]}
            uptime=""
            menuOpen
            marks={{ menu: n("menu") }}
          />
          <ArtRow
            compact
            port={4567}
            open
            name="ruby"
            folder={["helpdesk", "~/Projects"]}
            uptime=""
          />
          <ArtRow
            compact
            port={5173}
            hint="Vite"
            name="node"
            folder={["storefront", "~/Projects"]}
            uptime=""
          />
          <ArtRow
            compact
            port={8080}
            hint="HTTP alt"
            name="node"
            folder={["scratch", OUTSIDE_HOME[platform]]}
            uptime=""
          />
        </ArtTable>
        {/* The foot of another row's menu, for what a project in the home
            folder has none of: an item that cannot be used. */}
        <ArtMenu className="w-56 self-end">
          <ArtMenuHeading>node · port 8080</ArtMenuHeading>
          <ArtOmitted />
          <ArtMenuItem
            icon={TrashIcon}
            label={`Move to ${trashName(platform)}…`}
            destructive
            disabled
            reason="This folder is outside your home folder."
            mark={n("reason")}
            markAt="left"
          />
        </ArtMenu>
      </div>
      {/* The menu as far as what keeps track of a row. The page on stopping
          draws the rest of it. */}
      <ArtMenu>
        <ArtMenuHeading>node · port 3000</ArtMenuHeading>
        <ArtSeparator />
        <ArtMenuItem
          icon={ExternalLinkIcon}
          label="Open in Browser"
          shortcut={say(keys.browser)}
          mark={n("open")}
          markAt="left"
        />
        <ArtMenuItem
          icon={LinkIcon}
          label="Copy URL"
          shortcut={say(keys.copyUrl)}
        />
        <ArtSeparator />
        <ArtMenuItem
          icon={CodeIcon}
          label={`Open in ${editorName(context.editor)}`}
          shortcut={say(keys.editor)}
          mark={n("project")}
          markAt="left"
        />
        <ArtMenuItem icon={TerminalIcon} label="Open in Terminal" />
        <ArtMenuItem icon={FolderOpenIcon} label={revealLabel(platform)} />
        <ArtMenuItem icon={CopyIcon} label="Copy Folder Path" />
        <ArtMenuItem
          icon={SquareTerminalIcon}
          label="Copy Command"
          mark={n("command")}
          markAt="left"
        />
        <ArtSeparator />
        <ArtMenuItem
          icon={PinIcon}
          label="Pin Project"
          mark={n("track")}
          markAt="left"
        />
        <ArtMenuItem icon={EyeIcon} label="Watch Port 3000" />
        <ArtMenuItem icon={HistoryIcon} label="Port 3000 History" />
        <ArtOmitted />
      </ArtMenu>
    </div>
  );
}

// What each platform runs that a developer has met on a port.
const SYSTEM_EXAMPLE = {
  macos: { port: 5000, name: "ControlCenter", kind: "Apple System" },
  windows: { port: 135, name: "svchost", kind: "Microsoft System" },
  linux: { port: 22, name: "sshd", kind: "Distro System" },
  unknown: { port: 22, name: "sshd", kind: "System" },
} as const;

function StopArt({ n, context }: PictureProps) {
  const { platform } = context;
  const keys = welcomeKeys(platform);
  const system = SYSTEM_EXAMPLE[platform];
  return (
    // The menu and the dialog side by side where the guide is wide enough,
    // and one over the other where it is not.
    <div className="grid grid-cols-[minmax(0,1fr)_13rem] gap-x-5 gap-y-2 @[38rem]:grid-cols-[minmax(0,1fr)_9.5rem_11.5rem]">
      <div className="row-span-2 flex min-w-0 flex-col gap-2 @[38rem]:row-span-1">
        <ArtTable>
          <ArtRow
            compact
            port={3000}
            name="node"
            folder={["dashboard", "~/Projects"]}
            uptime=""
            checked
          />
          <ArtRow
            compact
            port={8000}
            name="Python"
            folder={["billing-api", "~/Projects"]}
            uptime=""
            checked
          />
          <ArtRow
            compact
            port={system.port}
            name={system.name}
            uptime=""
            kind={system.kind}
            locked
            marks={{ row: n("locked") }}
          />
        </ArtTable>
        <Marked number={n("several")} className="self-center">
          <span className="inline-flex items-center gap-1 rounded-full border bg-popover py-0.5 pr-0.5 pl-2.5 text-popover-foreground shadow-md">
            <span className="mr-1 tabular-nums">2 selected</span>
            <ArtButton variant="destructive" className="rounded-full">
              Stop
            </ArtButton>
            <ArtButton variant="ghost" className="rounded-full">
              Clear
            </ArtButton>
          </span>
        </Marked>
      </div>
      {/* The foot of a row's menu, on a port that two processes hold. */}
      <div className="min-w-0">
        <ArtMenu>
          <ArtMenuHeading>node · port 3000</ArtMenuHeading>
          <ArtOmitted />
          <ArtMenuItem
            icon={OctagonIcon}
            label="Stop…"
            shortcut={chordText(keys.stop, platform)}
            mark={n("stop")}
            markAt="left"
          />
          <ArtMenuItem
            icon={OctagonXIcon}
            label="Free Port 3000…"
            mark={n("free")}
            markAt="left"
          />
          <ArtSeparator />
          <ArtMenuItem
            icon={TrashIcon}
            label={`Move to ${trashName(platform)}…`}
            destructive
            mark={n("trash")}
            markAt="left"
          />
        </ArtMenu>
      </div>
      <div className="min-w-0">
        <Marked number={n("confirm")} at="left-edge" className="flex">
          <span className="flex min-w-0 flex-1 flex-col gap-1 rounded-md border bg-background p-2 shadow-md">
            <span className="font-semibold">Stop node on port 3000?</span>
            <span className="line-clamp-2 text-[9px] leading-[12px] text-muted-foreground">
              {stopProcessDescription(48211)}
            </span>
            <span className="mt-0.5 flex justify-end gap-1">
              <ArtButton>Cancel</ArtButton>
              <ArtButton variant="destructive">Stop Process</ArtButton>
            </span>
          </span>
        </Marked>
      </div>
    </div>
  );
}

function TrackArt({ n }: PictureProps) {
  return (
    <div className="grid grid-cols-[11rem_minmax(0,1fr)] gap-x-6">
      <div className="flex flex-col items-end gap-1">
        <div className="flex gap-1">
          <ArtToolButton icon={RefreshCwIcon} />
          <Marked number={n("bell")}>
            <ArtToolButton icon={BellIcon} active />
          </Marked>
          <ArtToolButton icon={EllipsisIcon} />
          <ArtToolButton icon={SettingsIcon} />
        </div>
        <ArtMenu className="w-full">
          <span className="flex flex-col px-1.5 py-px">
            <span className="font-medium">Port change toasts</span>
            <span className="text-[9px] leading-[11px] text-muted-foreground">
              On
            </span>
          </span>
          <ArtSeparator />
          <ArtMenuItem icon={ListXIcon} label="Clear all notifications" />
          <ArtSeparator />
          <ArtMenuItem icon={ClockIcon} label="Mute for 15 minutes" />
          <ArtMenuItem icon={ClockIcon} label="Mute for 1 hour" />
          <ArtMenuItem icon={ClockIcon} label="Mute for 8 hours" />
          <ArtMenuItem icon={BellOffIcon} label="Turn off" />
        </ArtMenu>
      </div>
      <div className="flex min-w-0 flex-col justify-between gap-2">
        {/* A toast, as the window draws one, and the strip under it. */}
        <Marked number={n("toast")} at="left-edge" className="flex">
          <span className="flex min-w-0 flex-1 flex-col gap-1">
            <span className="flex items-start gap-1.5 rounded-lg border border-sky-500/30 bg-sky-500/10 px-2 py-1.5 text-sky-700 dark:text-sky-300">
              <InfoIcon className="mt-px size-3 shrink-0" />
              <span className="flex min-w-0 flex-col">
                <span className="font-medium">Port change detected</span>
                <span className="truncate">
                  Port 5173 is now in use by node (PID 48377)
                </span>
              </span>
            </span>
            <span className="flex items-center justify-between rounded-md border bg-popover px-2 py-px text-[9px] text-popover-foreground">
              <span className="font-medium">Clear all</span>
              <span className="flex gap-1.5">
                <span className="text-muted-foreground">Mute for</span>
                <span className="font-medium">15m</span>
                <span className="font-medium">1h</span>
                <span className="font-medium">8h</span>
              </span>
              <span className="font-medium">Turn off</span>
            </span>
          </span>
        </Marked>
        {/* A notification, as the desktop draws one. */}
        <Marked number={n("watch")} at="left-edge" className="flex">
          <span className="flex min-w-0 flex-1 items-center gap-2 rounded-xl border bg-muted/60 p-2 shadow-md">
            <img src={appIcon} alt="" className="size-6 shrink-0" />
            <span className="flex min-w-0 flex-col">
              <span className="font-semibold">Port 5173 is now in use</span>
              <span className="truncate text-muted-foreground">
                node (PID 48377)
              </span>
            </span>
          </span>
        </Marked>
        {/* Two lines of a port's timeline. */}
        <Marked number={n("history")} at="left-edge" className="flex">
          <span className="flex min-w-0 flex-1 flex-col gap-1">
            <ArtHistoryLine kind="occupied" time="10:42 AM" />
            <ArtHistoryLine kind="freed" time="10:57 AM" />
          </span>
        </Marked>
      </div>
    </div>
  );
}

function ArtHistoryLine({
  kind,
  time,
}: {
  kind: "occupied" | "freed";
  time: string;
}) {
  return (
    <span className="flex items-center gap-1.5 rounded-md border bg-background px-1.5 py-0.5">
      <span
        className={cn(
          "rounded-full border px-1 text-[7px] leading-[11px] font-medium uppercase",
          kind === "occupied"
            ? "border-emerald-500/40 text-emerald-600 dark:text-emerald-400"
            : "border-sky-500/40 text-sky-600 dark:text-sky-400",
        )}
      >
        {kind}
      </span>
      <span className="truncate text-[9px] text-muted-foreground">
        {time} — {kind} by node (PID 48377)
      </span>
    </span>
  );
}

function BeyondArt({ n, context }: PictureProps) {
  const { platform } = context;
  const windows = platform === "windows";
  return (
    <div className="grid grid-cols-[10.5rem_minmax(0,1fr)_11.5rem] gap-x-5">
      {/* The menu under the app's icon, which the system draws. */}
      <ArtMenu className="self-start">
        <span className="relative flex px-1.5 py-px text-muted-foreground">
          Port Watch — 2 listeners
          <Pin number={n("tray")} at="left" />
        </span>
        <ArtSeparator />
        <ArtMenuItem label="3000  ·  node" submenu />
        <ArtMenuItem label="5173  ·  node" submenu />
        <ArtSeparator />
        <ArtMenuItem
          label="Open Full Window"
          mark={n("running")}
          markAt="left"
        />
        <ArtMenuItem label="Refresh" />
        <ArtMenuItem label="Menu bar mode" />
        <ArtSeparator />
        <ArtMenuItem label="Quit" />
      </ArtMenu>
      <Marked number={n("cli")} className="flex min-w-0 self-start">
        <span className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-md border border-zinc-700 bg-zinc-950 font-mono text-[9px] leading-[14px] text-zinc-300 shadow-md">
          <span className="flex gap-1 border-b border-zinc-800 px-2 py-1">
            <span className="size-1.5 rounded-full bg-zinc-700" />
            <span className="size-1.5 rounded-full bg-zinc-700" />
            <span className="size-1.5 rounded-full bg-zinc-700" />
          </span>
          <span className="flex flex-col px-2 py-1.5">
            <ArtCommand windows={windows}>port-watch check 3000</ArtCommand>
            <span className="truncate text-zinc-500">
              {'[{"id":"…","pid":48211,"name":"node",…}]'}
            </span>
            <ArtCommand windows={windows}>
              {windows ? "echo %errorlevel%" : "echo $?"}
            </ArtCommand>
            <span>1</span>
            <ArtCommand windows={windows}>port-watch check 4000</ArtCommand>
            <ArtCommand windows={windows}>
              {windows ? "echo %errorlevel%" : "echo $?"}
            </ArtCommand>
            <span>0</span>
          </span>
        </span>
      </Marked>
      <div className="flex min-w-0 flex-col items-end gap-1">
        <div className="flex gap-1">
          <ArtToolButton icon={RefreshCwIcon} />
          <ArtToolButton icon={BellIcon} />
          <ArtToolButton icon={EllipsisIcon} active />
          <Marked number={n("settings")}>
            <ArtToolButton icon={SettingsIcon} />
          </Marked>
        </div>
        {/* The toolbar's "…" menu. */}
        <ArtMenu className="w-full">
          <span className="px-1.5 py-px font-medium">
            Export the rows shown
          </span>
          <ArtMenuItem
            icon={BracesIcon}
            label="Copy as JSON"
            mark={n("export")}
            markAt="left"
          />
          <ArtMenuItem icon={FileTextIcon} label="Copy as Markdown" />
          <ArtSeparator />
          <ArtMenuItem icon={HistoryIcon} label="Port History…" />
          <ArtSeparator />
          <ArtMenuItem
            icon={BookOpenIcon}
            label="Welcome Guide…"
            mark={n("guide")}
            markAt="left"
          />
          <ArtMenuItem
            icon={KeyboardIcon}
            label="Keyboard Shortcuts…"
            shortcut={chordText(welcomeKeys(platform).guide, platform)}
          />
          <ArtSeparator />
          <ArtMenuItem
            icon={OctagonIcon}
            label="Stop all user processes shown…"
            destructive
          />
        </ArtMenu>
      </div>
    </div>
  );
}

/** A line typed at a prompt. */
function ArtCommand({
  windows,
  children,
}: {
  windows: boolean;
  children: ReactNode;
}) {
  return (
    <span className="truncate">
      <span className="text-zinc-500">{windows ? ">" : "$"}</span> {children}
    </span>
  );
}

const PICTURES: Partial<Record<WelcomePageId, ComponentType<PictureProps>>> = {
  list: ListArt,
  find: FindArt,
  act: ActArt,
  stop: StopArt,
  track: TrackArt,
  beyond: BeyondArt,
};

/** A page's picture, or nothing for a page that has none. */
export function WelcomeArt({
  page,
  context,
}: {
  page: WelcomePage;
  context: WelcomeContext;
}) {
  const Picture = PICTURES[page.id];
  if (!Picture) {
    return null;
  }
  const numbers = new Map(
    page.tips.map((tip, index) => [tip.mark, index + 1] as const),
  );

  return (
    // The cards under it say all of it in words.
    <div
      aria-hidden
      data-welcome-art={page.id}
      className={cn(
        "rounded-xl bg-linear-to-br px-8 py-3.5",
        WELCOME_TINTS[page.tint].art,
      )}
    >
      <div className="@container rounded-lg border bg-background p-3 text-[10px] leading-[14px] shadow-lg select-none">
        <Picture n={(mark) => numbers.get(mark)} context={context} />
      </div>
    </div>
  );
}
