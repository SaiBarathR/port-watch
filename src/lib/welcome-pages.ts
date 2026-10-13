import type { getPlatform } from "@/lib/platform";
import type { PreferredEditor } from "@/lib/types";
import type { WelcomePageId } from "@/lib/welcome";

// What the guide says. Words only: the pictures are in welcome-art.tsx, and
// each numbered dot on one is a tip here, found by its `mark`.
//
// Every sentence is a claim about the app. One that stops being true is a
// bug, so each says what a control is called on screen, in **bold**, and can
// be read against the code that draws it.

export type Platform = ReturnType<typeof getPlatform>;

/** One key or one chord, key by key: `["⌘", "R"]`. */
export type Chord = string[];

/** One thing that can be done on a page, and the keys for it. */
export interface WelcomeTip {
  /** Names the dot on the page's picture that points at what this is about. */
  mark: string;
  title: string;
  /** `**bold**` for what a control is called on screen, backticks for something to type. */
  text: string;
  keys?: Chord[];
}

/** A part of the window, on the map of it beside a page's heading. */
export type WindowRegion = "search" | "buttons" | "status" | "table";

export type WelcomeTint =
  "teal" | "sky" | "violet" | "rose" | "amber" | "indigo" | "zinc";

export interface WelcomePage {
  id: WelcomePageId;
  /** What the page is called in the list of them. */
  title: string;
  /** The colour of the page's symbol in the list, and of its picture. */
  tint: WelcomeTint;
  headline: string;
  lead: string;
  /** Where in the window the page is about. Null for one about no one part. */
  place: string | null;
  regions: WindowRegion[];
  /** Numbered in this order, which is how the picture numbers them. */
  tips: WelcomeTip[];
}

/** A line of the shortcuts page: the keys, and what they do. */
export interface WelcomeShortcut {
  keys: Chord[];
  action: string;
}

export interface WelcomeShortcutGroup {
  title: string;
  shortcuts: WelcomeShortcut[];
}

/** What the words depend on: the keys and names differ by platform. */
export interface WelcomeContext {
  platform: Platform;
  editor: PreferredEditor;
}

export interface WelcomeKeys {
  find: Chord;
  refresh: Chord;
  rowMenu: Chord;
  /** The keys a keyboard has for a menu of its own, which open it too. */
  rowMenuToo: Chord[];
  browser: Chord;
  editor: Chord;
  copyUrl: Chord;
  stop: Chord;
  /** With the key that deletes forwards, which stops as well. */
  stopToo: Chord;
  /** Held with a row's menu open, it shows the alternate items. */
  alternate: Chord;
  settings: Chord;
  guide: Chord;
}

export function welcomeKeys(platform: Platform): WelcomeKeys {
  return platform === "macos"
    ? {
        find: ["⌘", "F"],
        refresh: ["⌘", "R"],
        rowMenu: ["⌘", "K"],
        rowMenuToo: [["⇧", "F10"]],
        browser: ["↵"],
        editor: ["⌘", "O"],
        copyUrl: ["⇧", "⌘", "C"],
        stop: ["⌘", "⌫"],
        stopToo: ["⌘", "⌦"],
        alternate: ["⌥"],
        settings: ["⌘", ","],
        guide: ["⌘", "/"],
      }
    : {
        find: ["Ctrl", "F"],
        refresh: ["Ctrl", "R"],
        rowMenu: ["Ctrl", "K"],
        rowMenuToo: [["Shift", "F10"], ["Menu"]],
        browser: ["Enter"],
        editor: ["Ctrl", "O"],
        copyUrl: ["Ctrl", "Shift", "C"],
        stop: ["Ctrl", "Backspace"],
        stopToo: ["Ctrl", "Delete"],
        alternate: ["Shift"],
        settings: ["Ctrl", ","],
        guide: ["Ctrl", "/"],
      };
}

/** A chord as it is written in a sentence: "⌘R", or "Ctrl+R". */
export function chordText(chord: Chord, platform: Platform): string {
  return chord.join(platform === "macos" ? "" : "+");
}

const SPOKEN: Record<string, string> = {
  "⌘": "Command",
  "⇧": "Shift",
  "⌥": "Option",
  "⌫": "Delete",
  "⌦": "Forward Delete",
  "↵": "Return",
  "↑": "Up Arrow",
  "↓": "Down Arrow",
  "←": "Left Arrow",
  "→": "Right Arrow",
  "/": "Slash",
  ",": "Comma",
  esc: "Escape",
  space: "Space",
};

/** The names of the keys the symbols stand for, for being read out. */
export function chordSpoken(chord: Chord): string {
  return chord.map((key) => SPOKEN[key] ?? key).join(" ");
}

export function editorName(editor: PreferredEditor): string {
  return editor === "code" ? "VS Code" : "Cursor";
}

export function revealLabel(platform: Platform): string {
  return platform === "macos"
    ? "Reveal in Finder"
    : platform === "windows"
      ? "Show in Explorer"
      : "Show in File Manager";
}

/** Where a removed folder goes, as the row's menu names it. */
export function trashName(platform: Platform): string {
  return platform === "windows" ? "Recycle Bin" : "Trash";
}

/** Where the app's icon sits when its window is shut. */
export function trayName(platform: Platform): string {
  return platform === "macos" ? "menu bar" : "system tray";
}

function osName(platform: Platform): string {
  switch (platform) {
    case "macos":
      return "macOS";
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    default:
      return "the system";
  }
}

export function welcomePages({
  platform,
  editor,
}: WelcomeContext): WelcomePage[] {
  const mac = platform === "macos";
  const keys = welcomeKeys(platform);
  const say = (chord: Chord) => chordText(chord, platform);
  const enter = mac ? "Return" : "Enter";
  const tray = trayName(platform);
  const trash = trashName(platform);

  return [
    {
      id: "list",
      title: "What Is Listening",
      tint: "teal",
      headline: "See what is listening",
      lead:
        "Every port on this machine that a program is listening on, and the " +
        "program behind it. Your own come first; what the system runs is one " +
        "click away.",
      place: "The table, and the line above it",
      regions: ["status", "table"],
      tips: [
        {
          mark: "row",
          title: "One row, one program",
          text:
            "The port leads, then the process, its PID, what was run, its " +
            "project and its uptime; **System** and **All** add its user and " +
            "a type. Several ports, still one row.",
        },
        {
          mark: "reach",
          title: "Who can reach it",
          text:
            "The mark after a port is a grey lock when only this machine can " +
            "connect, and amber when other machines can. Hover over it for " +
            "the addresses.",
        },
        {
          mark: "scope",
          title: "Yours, the system’s, or all",
          text:
            "**User** is what you run yourself. **System** is the rest: " +
            `${osName(platform)}’s own programs, and what other users run. ` +
            "**All** is both, each with its count.",
        },
        {
          mark: "live",
          title: "Kept up to date",
          text:
            `Scanned again every few seconds; ${say(keys.refresh)} scans now. ` +
            "The line on the right says if the list is live, paused under an " +
            "open menu or dialog, or off.",
          keys: [keys.refresh],
        },
        {
          mark: "change",
          title: "What just changed",
          text:
            "A listener that has just appeared is tinted green and marked " +
            "**NEW** for ten seconds; one whose ports changed is amber and " +
            "marked **CHANGED**.",
        },
        {
          mark: "pinned",
          title: "Pin and group",
          text:
            "**Pin Project** in a row’s menu keeps a project at the top, " +
            "under **Pinned**. **Group by project** in Settings keeps rows " +
            "from one folder together.",
        },
      ],
    },
    {
      id: "find",
      title: "Find a Port",
      tint: "sky",
      headline: "Find a port, and who has it",
      lead:
        "The search box narrows the list as you type. Asked about one port, " +
        "it answers on the line underneath.",
      place: "The top of the window",
      regions: ["search", "status"],
      tips: [
        {
          mark: "search",
          title: "Search",
          text:
            `${say(keys.find)} or / goes to the search box. It looks in ports, ` +
            "processes, PIDs, users, paths and commands. Esc empties it.",
          keys: [keys.find, ["/"]],
        },
        {
          mark: "field",
          title: "One field at a time",
          text:
            "The menu at the start of the box narrows the search to **Port**, " +
            "**PID**, **Process**, **User**, **Path** or **Command**.",
        },
        {
          mark: "answer",
          title: "Ask about one port",
          text:
            "Search by **Port** and type a whole port number. The line below " +
            "says who holds it, or that it is free, and who last held it if " +
            "that was seen.",
        },
        {
          mark: "free",
          title: "Free it",
          text:
            "**Free port** stops every process on that port that can be " +
            "stopped, after asking. **History** shows what has come and gone " +
            "on it.",
        },
        {
          mark: "rows",
          title: "Into the rows",
          text:
            "↓ from the search box goes into the rows; ↑ and ↓ move between " +
            "them. Tab stops at the rows once, not at each one.",
          keys: [["↓"]],
        },
      ],
    },
    {
      id: "act",
      title: "Work on a Row",
      tint: "violet",
      headline: "Everything a row can do",
      lead:
        "A menu on every row: open what is listening there, go to its " +
        "project, keep an eye on it.",
      place: "Any row",
      regions: ["table"],
      tips: [
        {
          mark: "menu",
          title: "Open a row’s menu",
          text:
            "Right-click a row, or press the **…** at its end. " +
            `${say(keys.rowMenu)} opens the menu of the row that has the keys.`,
          keys: [keys.rowMenu],
        },
        {
          mark: "open",
          title: "Open what it serves",
          text:
            "**Open in Browser** opens localhost on that port, and **Copy " +
            `URL** copies its address. ${enter} and ${say(keys.copyUrl)} do ` +
            "the same from a row.",
          keys: [keys.browser, keys.copyUrl],
        },
        {
          mark: "project",
          title: "Go to its project",
          text:
            `**Open in ${editorName(editor)}** and **Open in Terminal** open ` +
            `the project folder. **${revealLabel(platform)}** shows it, as ` +
            "does clicking it in the **Project** column.",
          keys: [keys.editor],
        },
        {
          mark: "command",
          title: "Copy what was run",
          text:
            "**Copy Command** copies the whole command line, which the row " +
            "cuts short. **Copy Folder Path** copies where the project is.",
        },
        {
          mark: "track",
          title: "Pin it, watch it",
          text:
            "**Pin Project** keeps the row at the top of the list. **Watch " +
            "Port** adds its port to the ones you can be alerted about.",
        },
        {
          mark: "reason",
          title: "When something can’t be done",
          text:
            "The item stays in its place, greyed, and says why underneath: " +
            "that no folder is known for the process, say.",
        },
      ],
    },
    {
      id: "stop",
      title: "Stop and Remove",
      tint: "rose",
      headline: "Stop it, or remove the project",
      lead:
        "Stopping a process frees its port. Removing takes its project " +
        "folder as well. Both ask first, and what the system runs is locked " +
        "until you unlock it.",
      place: "A row’s menu, and the bar over the table",
      regions: ["table"],
      tips: [
        {
          mark: "stop",
          title: "Stop one",
          text:
            `**Stop…** in a row’s menu, or ${say(keys.stop)} on the row. ` +
            (platform === "windows"
              ? "The process is ended with taskkill, by force if it has to be."
              : "The process is asked to quit, then made to after two seconds."),
          keys: [keys.stop],
        },
        {
          mark: "confirm",
          title: "It asks first",
          text:
            "Every stop and every removal is confirmed in a dialog first. " +
            "Esc backs out of it.",
          keys: [["esc"]],
        },
        {
          mark: "several",
          title: "Stop several",
          text:
            "Tick rows, or press Space on one, and a bar over the table " +
            "offers **Stop** for all of them. The toolbar’s **…** menu can " +
            "stop every user process shown.",
          keys: [["space"]],
        },
        {
          mark: "free",
          title: "Free a shared port",
          text:
            "When another process holds a row’s first port too, its menu " +
            "adds **Free Port**, which stops all of them that can be stopped.",
        },
        {
          mark: "trash",
          title: "Remove the project",
          text:
            `**Move to ${trash}…** stops the process and moves its project ` +
            `folder there. Hold ${say(keys.alternate)} with the menu open for ` +
            "**Delete Permanently…**, which wants the folder’s name typed.",
          keys: [keys.alternate],
        },
        {
          mark: "locked",
          title: "What is protected",
          text:
            "A system service can’t be stopped until **Allow actions on " +
            "system services** is on in Settings; a stop then asks twice. " +
            "Only a folder inside your home folder can be removed.",
        },
      ],
    },
    {
      id: "track",
      title: "Keep Track",
      tint: "amber",
      headline: "Know when a port changes",
      lead:
        "Port Watch notes each port it sees taken or freed, from one scan " +
        "to the next, and can tell you as it happens.",
      place: "The bell, and the corner of the window",
      regions: ["buttons"],
      tips: [
        {
          mark: "toast",
          title: "Port change toasts",
          text:
            "A note in the corner when a port is taken or freed among the " +
            "listeners shown: yours, the system’s, or all. Changes close " +
            "together share one note.",
        },
        {
          mark: "bell",
          title: "Mute them, or turn them off",
          text:
            "The bell in the toolbar, like the strip under a note, clears " +
            "them, mutes them for 15 minutes, 1 hour or 8 hours, or turns " +
            "them off.",
        },
        {
          mark: "watch",
          title: "Watch a port",
          text:
            "Pick ports with **Watch Port** in a row’s menu, or in Settings. " +
            "With **Watched port alerts** on there, a desktop notification " +
            "says when one is taken, freed or changes hands.",
        },
        {
          mark: "history",
          title: "Port history",
          text:
            "**Port History…** in the toolbar’s **…** menu lists the ports " +
            "that have changed and opens one port’s timeline. The last 500 " +
            "changes are kept, on this machine.",
        },
      ],
    },
    {
      id: "beyond",
      title: "Outside the Window",
      tint: "indigo",
      headline: "Without the window",
      lead:
        `The list is in the ${tray} as well, and one port can be asked ` +
        "about from a terminal.",
      place: null,
      regions: [],
      tips: [
        {
          mark: "tray",
          title: mac ? "The menu bar" : "The system tray",
          text:
            `Port Watch’s icon in the ${tray} lists your dev servers. Each ` +
            "has a submenu to open it, copy its URL, open its folder in a " +
            "terminal or the editor, or stop it.",
        },
        {
          mark: "running",
          title: "It keeps running",
          text:
            `Closing the window leaves Port Watch in the ${tray}, where ` +
            "**Open Full Window** brings it back and **Quit** ends it." +
            (mac ? " **Menu bar mode** hides its Dock icon too." : ""),
        },
        {
          mark: "cli",
          title: "From a terminal",
          text:
            "`port-watch check 3000` exits with 0 if the port is free, or " +
            "with 1 and who holds it, as JSON. `--udp` counts UDP too. " +
            "Install the command from Settings.",
        },
        {
          mark: "export",
          title: "Copy what is shown",
          text:
            "**Copy as JSON** and **Copy as Markdown**, in the toolbar’s " +
            "**…** menu, copy the rows the search and the filter have left " +
            "in view.",
        },
        {
          mark: "settings",
          title: "Settings",
          text:
            "The theme, how often to scan, UDP, the editor that projects " +
            `open in, and the rest. ${say(keys.settings)} opens them.`,
          keys: [keys.settings],
        },
        {
          mark: "guide",
          title: "This guide",
          text:
            "**Welcome Guide…** in the toolbar’s **…** menu opens it again, " +
            `and ${say(keys.guide)} opens it at its list of keys.`,
          keys: [keys.guide],
        },
      ],
    },
    {
      id: "keys",
      title: "Keyboard Shortcuts",
      tint: "zinc",
      headline: "Keyboard shortcuts",
      lead:
        "The window’s keys, in one place. While this guide is up, only its " +
        "own are answered.",
      place: null,
      regions: [],
      tips: [],
    },
  ];
}

/**
 * The keys the window answers, in the order someone would come to need them.
 * The handlers are where these are set; this is where they are all in one
 * place, and a key a handler gains belongs here too.
 */
export function welcomeShortcuts(platform: Platform): WelcomeShortcutGroup[] {
  const keys = welcomeKeys(platform);
  const trash = trashName(platform);

  return [
    {
      title: "Finding",
      shortcuts: [
        { keys: [keys.find, ["/"]], action: "Go to the search box" },
        {
          keys: [keys.rowMenu],
          action: "The search box too, unless a row has the keys",
        },
        { keys: [["esc"]], action: "In the search box, empty it" },
        { keys: [["↓"]], action: "From the search box, into the rows" },
        { keys: [keys.refresh], action: "Scan now" },
      ],
    },
    {
      title: "In the table",
      shortcuts: [
        { keys: [["↑"], ["↓"]], action: "Move between rows" },
        { keys: [["Home"], ["End"]], action: "The first row, the last row" },
        { keys: [["space"]], action: "Tick the row’s box, or untick it" },
        {
          keys: [keys.rowMenu, ...keys.rowMenuToo],
          action: "Open the row’s menu",
        },
      ],
    },
    {
      title: "On a row",
      shortcuts: [
        { keys: [keys.browser], action: "Open in the browser" },
        { keys: [keys.editor], action: "Open the project in the editor" },
        { keys: [keys.copyUrl], action: "Copy the URL" },
        { keys: [keys.stop, keys.stopToo], action: "Stop, after asking" },
        {
          keys: [keys.alternate],
          action: `Held with the row’s menu open: Delete Permanently in place of Move to ${trash}`,
        },
      ],
    },
    {
      title: "The window",
      shortcuts: [
        { keys: [keys.settings], action: "Settings" },
        { keys: [["esc"]], action: "Close a menu or a dialog" },
      ],
    },
    {
      title: "This guide",
      shortcuts: [
        { keys: [keys.guide], action: "Open it at this page" },
        { keys: [["→"], ["←"]], action: "Turn its pages" },
        { keys: [["esc"]], action: "Shut it" },
      ],
    },
  ];
}
