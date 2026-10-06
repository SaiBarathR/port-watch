import { useEffect, useState, type ReactNode } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { AdvancedSettings } from "@/components/settings/advanced-settings";
import { GeneralSettings } from "@/components/settings/general-settings";
import { IntegrationSettings } from "@/components/settings/integration-settings";
import { NotificationSettings } from "@/components/settings/notification-settings";
import type { ThemeMode } from "@/hooks/use-theme";
import { isMacOS } from "@/lib/platform";
import type { AppSettings } from "@/lib/types";

interface SettingsDialogProps {
  settings: AppSettings;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  trigger: ReactNode;
}

export function SettingsDialog({
  settings,
  theme,
  onThemeChange,
  trigger,
}: SettingsDialogProps) {
  const [open, setOpen] = useState(false);

  // ⌘, is where a Mac app keeps its settings; Ctrl+, does the same elsewhere.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((isMacOS() ? event.metaKey : event.ctrlKey) && event.key === ",") {
        event.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent className="flex max-h-[min(85vh,720px)] w-[calc(100%-2rem)] max-w-lg flex-col gap-0 overflow-hidden p-0 sm:w-full">
        <DialogHeader className="shrink-0 border-b px-6 pt-6 pr-12 pb-4">
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>Changes apply as you make them.</DialogDescription>
        </DialogHeader>

        {/* Mounted only while open: the sections ask the backend about the
            command-line tool when they appear. */}
        {open && (
          <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-x-hidden overflow-y-auto px-6 py-5">
            <GeneralSettings
              settings={settings}
              theme={theme}
              onThemeChange={onThemeChange}
            />
            <NotificationSettings settings={settings} />
            <IntegrationSettings settings={settings} />
            <AdvancedSettings settings={settings} />
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
