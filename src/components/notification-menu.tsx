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
import type { AppSettings } from "@/lib/types";

interface NotificationMenuProps {
  settings: AppSettings;
  onShowChangeToastsChange: (show: boolean) => void;
  onChangeToastsMutedUntilChange: (mutedUntil: number | null) => void;
}

export function NotificationMenu({
  settings,
  onShowChangeToastsChange,
  onChangeToastsMutedUntilChange,
}: NotificationMenuProps) {
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
          <ListXIcon data-icon="inline-start" />
          Clear all notifications
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {status === "muted" && (
          <DropdownMenuItem onClick={() => onChangeToastsMutedUntilChange(null)}>
            <BellRingIcon data-icon="inline-start" />
            Unmute
          </DropdownMenuItem>
        )}
        {status === "on" &&
          MUTE_DURATIONS.map((duration) => (
            <DropdownMenuItem
              key={duration.ms}
              onClick={() =>
                muteChangeToasts(duration.ms, onChangeToastsMutedUntilChange)
              }
            >
              <ClockIcon data-icon="inline-start" />
              Mute for {duration.label}
            </DropdownMenuItem>
          ))}
        {status === "off" ? (
          <DropdownMenuItem onClick={() => onShowChangeToastsChange(true)}>
            <BellIcon data-icon="inline-start" />
            Turn on
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem
            onClick={() => turnOffChangeToasts(onShowChangeToastsChange)}
          >
            <BellOffIcon data-icon="inline-start" />
            Turn off
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
