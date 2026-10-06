import {
  BellIcon,
  BellOffIcon,
  BellRingIcon,
  ClockIcon,
  ListXIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  MUTE_DURATIONS,
  changeToastStatus,
  dismissAllToasts,
  formatMutedUntil,
  muteChangeToasts,
  turnOffChangeToasts,
} from "@/lib/change-toasts";
import {
  setChangeToastsMutedUntil,
  setShowChangeToasts,
} from "@/lib/settings-actions";
import type { AppSettings } from "@/lib/types";

interface NotificationMenuProps {
  settings: AppSettings;
}

export function NotificationMenu({ settings }: NotificationMenuProps) {
  // A mute runs out with the clock, not with a state change, so the clock
  // is read on every render.
  // eslint-disable-next-line react-hooks/purity
  const status = changeToastStatus(settings, Date.now());
  const statusLabel =
    status === "muted" && settings.changeToastsMutedUntil !== null
      ? `Muted until ${formatMutedUntil(settings.changeToastsMutedUntil)}`
      : status === "off"
        ? "Off"
        : "On";

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="outline"
          size="icon"
          className="size-9 shrink-0"
          aria-label={`Notifications: port change toasts ${statusLabel.toLowerCase()}`}
        >
          {status === "on" ? <BellIcon /> : <BellOffIcon />}
        </Button>
      </DropdownMenuTrigger>
      {/* Sits above the toast stack, which can reach the toolbar when it is full. */}
      <DropdownMenuContent align="end" className="z-[1000000000] w-56">
        <DropdownMenuLabel>
          Port change toasts
          <span className="block text-xs font-normal text-muted-foreground">
            {statusLabel}
          </span>
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={dismissAllToasts}>
          <ListXIcon />
          Clear all notifications
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {status === "muted" && (
          <DropdownMenuItem onClick={() => setChangeToastsMutedUntil(null)}>
            <BellRingIcon />
            Unmute
          </DropdownMenuItem>
        )}
        {status === "on" &&
          MUTE_DURATIONS.map((duration) => (
            <DropdownMenuItem
              key={duration.ms}
              onClick={() =>
                muteChangeToasts(duration.ms, setChangeToastsMutedUntil)
              }
            >
              <ClockIcon />
              Mute for {duration.label}
            </DropdownMenuItem>
          ))}
        {status === "off" ? (
          <DropdownMenuItem onClick={() => setShowChangeToasts(true)}>
            <BellIcon />
            Turn on
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem
            onClick={() => turnOffChangeToasts(setShowChangeToasts)}
          >
            <BellOffIcon />
            Turn off
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
