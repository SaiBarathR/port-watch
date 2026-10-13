import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  BellIcon,
  KeyboardIcon,
  MousePointerClickIcon,
  OctagonIcon,
  Rows3Icon,
  SearchIcon,
  SquareTerminalIcon,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  GuideBadge,
  WELCOME_TINTS,
  WelcomeArt,
} from "@/components/welcome-art";
import { getPlatform, isMacOS } from "@/lib/platform";
import { focusTabStopRow } from "@/lib/row-focus";
import { useSettings } from "@/lib/settings-store";
import {
  closeWelcome,
  isWelcomeOpen,
  opensWithWelcome,
  setOpensWithWelcome,
  showWelcome,
  stepWelcome,
  turnWelcomeTo,
  useWelcome,
  type WelcomePageId,
} from "@/lib/welcome";
import {
  chordSpoken,
  welcomePages,
  welcomeShortcuts,
  type Chord,
  type WelcomeContext,
  type WelcomePage,
  type WelcomeShortcutGroup,
  type WelcomeTip,
  type WindowRegion,
} from "@/lib/welcome-pages";
import { cn } from "@/lib/utils";
import { version } from "../../package.json";
import appIcon from "../../src-tauri/icons/128x128@2x.png";

const PAGE_ICONS: Record<WelcomePageId, LucideIcon> = {
  list: Rows3Icon,
  find: SearchIcon,
  act: MousePointerClickIcon,
  stop: OctagonIcon,
  track: BellIcon,
  beyond: SquareTerminalIcon,
  keys: KeyboardIcon,
};

/**
 * The guide to the app: what it does, where each thing is, and the keys.
 *
 * Put up over the window when the app opens, until it is asked to stay
 * away, and from the toolbar's "…" menu after that. There is a good deal in
 * the app that nothing on screen gives away — that Space selects a row, that
 * a port typed into the search box is answered on the line under it, that
 * Option turns Move to Trash into Delete Permanently — and it was all in a
 * README.
 */
export function WelcomeGuide() {
  const { open, page: pageId } = useWelcome();
  const { preferredEditor } = useSettings();
  const platform = getPlatform();
  const context = useMemo<WelcomeContext>(
    () => ({ platform, editor: preferredEditor }),
    [platform, preferredEditor],
  );
  const pages = useMemo(() => welcomePages(context), [context]);
  const index = Math.max(
    0,
    pages.findIndex((entry) => entry.id === pageId),
  );
  const page = pages[index];
  const isFirst = index === 0;
  const isLast = index === pages.length - 1;

  // The box at the foot of it. Held here as well as in storage, so the tick
  // is drawn from what was just clicked.
  const [staysAway, setStaysAway] = useState(() => !opensWithWelcome());
  const staysAwayId = useId();

  const forwardRef = useRef<HTMLButtonElement>(null);
  // What had the keyboard when the guide went up, to hand it back to.
  const returnFocusRef = useRef<Element | null>(null);

  // The key that opens the guide at the list of keys. Heard before anything
  // else hears it: while the guide is up it keeps the keys from the window
  // behind it, and this one is its own.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        !(isMacOS() ? event.metaKey : event.ctrlKey) ||
        event.altKey ||
        event.key !== "/"
      ) {
        return;
      }
      // Not over a dialog that is waiting for an answer.
      if (
        !isWelcomeOpen() &&
        document.querySelector('[role="dialog"], [role="alertdialog"]')
      ) {
        return;
      }
      event.preventDefault();
      showWelcome("keys");
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);

  // Back is switched off on the first page and Close is gone from the last.
  // A button that had the keyboard when a turn of the page did that to it
  // takes the keyboard with it, and the arrow keys with that: it goes to the
  // button that is on every page.
  const turnedToRef = useRef(index);
  useLayoutEffect(() => {
    if (turnedToRef.current === index) {
      return;
    }
    turnedToRef.current = index;
    const active = document.activeElement;
    if (
      active === null ||
      active === document.body ||
      (active instanceof HTMLButtonElement && active.disabled)
    ) {
      forwardRef.current?.focus();
    }
  }, [index]);

  return (
    <Dialog open={open} onOpenChange={(next) => !next && closeWelcome()}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(calc(100vh-2rem),48rem)] w-[calc(100%-2rem)] max-w-5xl flex-col gap-0 overflow-hidden p-0"
        onOpenAutoFocus={(event) => {
          // On the button that turns the page, so Return reads the guide
          // through and then shuts it.
          event.preventDefault();
          returnFocusRef.current = document.activeElement;
          forwardRef.current?.focus();
        }}
        onCloseAutoFocus={(event) => {
          // Opened at launch, or from a menu item that has since gone:
          // there is no button to hand the keyboard back to, and the rows
          // are where it is of use.
          event.preventDefault();
          const from = returnFocusRef.current;
          if (
            from instanceof HTMLElement &&
            from !== document.body &&
            from.isConnected
          ) {
            from.focus();
          } else {
            focusTabStopRow();
          }
        }}
        onKeyDown={(event) => {
          if (
            !event.metaKey &&
            !event.ctrlKey &&
            !event.altKey &&
            !event.shiftKey
          ) {
            if (event.key === "ArrowRight") {
              event.preventDefault();
              stepWelcome(1);
            } else if (event.key === "ArrowLeft") {
              event.preventDefault();
              stepWelcome(-1);
            }
          }
          // The window's own keys are for the window, which this is over:
          // without this, the search key took the keyboard out from under
          // the guide and the settings key opened Settings on top of it.
          event.stopPropagation();
        }}
      >
        <div className="flex min-h-0 flex-1">
          <aside className="flex w-52 shrink-0 flex-col border-r bg-muted/30">
            <div className="flex flex-col gap-1 px-4 pt-5 pb-4">
              <img src={appIcon} alt="" className="-ml-1 size-12" />
              <DialogTitle className="mt-1 text-[15px] leading-tight">
                Welcome to Port Watch
              </DialogTitle>
              <DialogDescription className="text-xs">
                What it does, and where everything is
              </DialogDescription>
            </div>

            <nav aria-label="Pages of the guide" className="px-2">
              <ul className="flex flex-col gap-0.5">
                {pages.map((entry) => (
                  <li key={entry.id}>
                    <PageButton
                      page={entry}
                      current={entry.id === page.id}
                      onSelect={() => turnWelcomeTo(entry.id)}
                    />
                  </li>
                ))}
              </ul>
            </nav>

            <p className="mt-auto p-4 text-[10px] text-muted-foreground/70">
              Port Watch {version}
            </p>
          </aside>

          {/* A new page starts at its top, not where the last was scrolled
              to. */}
          <PageView key={page.id} page={page} context={context} />
        </div>

        <footer className="flex shrink-0 items-center gap-3 border-t px-4 py-3">
          <label
            htmlFor={staysAwayId}
            className="flex items-center gap-2 text-sm"
          >
            <input
              id={staysAwayId}
              type="checkbox"
              className="size-4 accent-primary"
              checked={staysAway}
              onChange={(event) => {
                setStaysAway(event.target.checked);
                setOpensWithWelcome(!event.target.checked);
              }}
            />
            Don’t show this again
          </label>
          {/* Said, and not left to be found out: it is what makes the box
              safe to tick. */}
          <span className="hidden text-xs text-muted-foreground md:inline">
            It stays in the toolbar’s … menu.
          </span>

          <span className="ml-auto text-xs text-muted-foreground tabular-nums">
            {index + 1} of {pages.length}
          </span>
          <Button
            variant="outline"
            size="sm"
            disabled={isFirst}
            onClick={() => stepWelcome(-1)}
          >
            Back
          </Button>
          {!isLast && (
            <Button variant="outline" size="sm" onClick={closeWelcome}>
              Close
            </Button>
          )}
          {/* One button with new words on the last page, so the keyboard
              stays on it from the first page to the end. */}
          <Button
            ref={forwardRef}
            size="sm"
            onClick={isLast ? closeWelcome : () => stepWelcome(1)}
          >
            {isLast ? "Get Started" : "Next"}
          </Button>
        </footer>
      </DialogContent>
    </Dialog>
  );
}

function PageButton({
  page,
  current,
  onSelect,
}: {
  page: WelcomePage;
  current: boolean;
  onSelect: () => void;
}) {
  const Icon = PAGE_ICONS[page.id];
  return (
    <button
      type="button"
      aria-current={current ? "page" : undefined}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] outline-none transition-colors hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring",
        current && "bg-accent font-medium",
      )}
      onClick={onSelect}
    >
      <span
        className={cn(
          "inline-flex size-[22px] shrink-0 items-center justify-center rounded-md bg-linear-to-b text-white",
          WELCOME_TINTS[page.tint].tile,
        )}
      >
        <Icon aria-hidden className="size-3" strokeWidth={2.25} />
      </span>
      <span className="truncate">{page.title}</span>
    </button>
  );
}

/**
 * What one page says: a picture of the part of the app it is about, where
 * that is, and the things to do there.
 */
function PageView({
  page,
  context,
}: {
  page: WelcomePage;
  context: WelcomeContext;
}) {
  const headingId = useId();
  return (
    <section
      aria-labelledby={headingId}
      className="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto px-6 pt-4 pb-3"
    >
      <WelcomeArt page={page} context={context} />

      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1 space-y-1">
          <h3
            id={headingId}
            className="text-xl leading-tight font-semibold tracking-tight"
          >
            {page.headline}
          </h3>
          <p className="text-sm text-muted-foreground">{page.lead}</p>
        </div>
        {page.place && (
          <figure className="flex w-28 shrink-0 flex-col items-center gap-1">
            <WindowMap lit={page.regions} />
            <figcaption className="text-center text-[10px] leading-tight text-muted-foreground">
              {page.place}
            </figcaption>
          </figure>
        )}
      </div>

      {page.id === "keys" ? (
        <ShortcutsTable groups={welcomeShortcuts(context.platform)} />
      ) : (
        <ol className="grid grid-cols-2 gap-2.5">
          {page.tips.map((tip, index) => (
            <TipCard key={tip.mark} tip={tip} number={index + 1} />
          ))}
        </ol>
      )}
    </section>
  );
}

/** One thing to do, under the number the picture points at it with. */
function TipCard({ tip, number }: { tip: WelcomeTip; number: number }) {
  return (
    <li className="flex items-start gap-2.5 rounded-lg border bg-muted/30 p-3">
      <GuideBadge number={number} className="mt-px ring-0" />
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-start justify-between gap-2">
          <h4 className="text-[13px] leading-snug font-semibold">
            {tip.title}
          </h4>
          {tip.keys && <Keys chords={tip.keys} />}
        </div>
        <p className="text-xs leading-[1.5] text-muted-foreground">
          <Rich text={tip.text} />
        </p>
      </div>
    </li>
  );
}

/**
 * A tip's words. `**bold**` is what a control is called on screen, and
 * backticks hold something to type.
 */
function Rich({ text }: { text: string }) {
  return text.split(/(\*\*[^*]+\*\*|`[^`]+`)/).map((part, index) =>
    part.startsWith("**") ? (
      <strong key={index} className="font-semibold text-foreground">
        {part.slice(2, -2)}
      </strong>
    ) : part.startsWith("`") ? (
      <code
        key={index}
        className="rounded bg-muted px-1 py-px font-mono text-[11px] text-foreground"
      >
        {part.slice(1, -1)}
      </code>
    ) : (
      part
    ),
  );
}

/** Keys and chords in a row: `⌘ R`, or `→  ←`. */
function Keys({ chords }: { chords: Chord[] }) {
  return (
    <span className="flex shrink-0 items-center gap-1.5">
      {chords.map((chord) => (
        <span key={chord.join(" ")} className="flex items-center gap-0.5">
          {/* The symbols are read out as their Unicode names, or not at
              all. */}
          <span className="sr-only">{chordSpoken(chord)}</span>
          {chord.map((key) => (
            <kbd
              key={key}
              aria-hidden
              className="inline-flex h-5 min-w-5 items-center justify-center rounded-[5px] border bg-background px-1 font-sans text-[11px] leading-none font-medium text-foreground shadow-[0_1px_0_var(--border)]"
            >
              {key}
            </kbd>
          ))}
        </span>
      ))}
    </span>
  );
}

/** Every key, under the heading of what it is for. */
function ShortcutsTable({ groups }: { groups: WelcomeShortcutGroup[] }) {
  // Two columns of about the same length.
  const halves = [groups.slice(0, 2), groups.slice(2)];
  return (
    <div className="grid grid-cols-2 items-start gap-x-4">
      {halves.map((half, column) => (
        <div key={column} className="flex flex-col gap-4">
          {half.map((group) => (
            <section key={group.title}>
              <h4 className="mb-1.5 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
                {group.title}
              </h4>
              <dl className="divide-y rounded-lg border bg-muted/30">
                {group.shortcuts.map((shortcut) => (
                  <div
                    key={shortcut.action}
                    className="flex items-center justify-between gap-3 px-3 py-1.5"
                  >
                    <dt className="order-last">
                      <Keys chords={shortcut.keys} />
                    </dt>
                    <dd className="text-xs">{shortcut.action}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
      ))}
    </div>
  );
}

/** The window, drawn small, with the part a page is about lit. */
function WindowMap({ lit }: { lit: WindowRegion[] }) {
  const part = (region: WindowRegion, className: string) => (
    <span
      className={cn(
        "rounded-[2px]",
        lit.includes(region) ? "bg-sky-600" : "bg-foreground/15",
        className,
      )}
    />
  );
  return (
    <span
      aria-hidden
      className="flex w-full flex-col gap-0.5 rounded-md border bg-muted/40 p-1.5"
    >
      <span className="flex gap-0.5">
        {part("search", "h-2 flex-1")}
        {part("buttons", "h-2 w-6")}
      </span>
      {part("status", "h-1.5")}
      {part("table", "h-6")}
    </span>
  );
}
