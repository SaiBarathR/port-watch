import { Fragment, type ComponentType, type ReactNode } from "react";
import {
  CodeIcon,
  CopyIcon,
  ExternalLinkIcon,
  EyeIcon,
  EyeOffIcon,
  FolderOpenIcon,
  HistoryIcon,
  LinkIcon,
  OctagonIcon,
  OctagonXIcon,
  PinIcon,
  PinOffIcon,
  TerminalIcon,
  Trash2Icon,
  TrashIcon,
  type LucideIcon,
} from "lucide-react";
import type { RowAction, RowActionId } from "@/lib/row-actions";

function iconFor(action: RowAction): LucideIcon {
  switch (action.id) {
    case "open-browser":
      return ExternalLinkIcon;
    case "copy-url":
      return LinkIcon;
    case "open-editor":
      return CodeIcon;
    case "open-terminal":
      return TerminalIcon;
    case "reveal":
      return FolderOpenIcon;
    case "copy-path":
      return CopyIcon;
    case "pin":
      return action.label.startsWith("Unpin") ? PinOffIcon : PinIcon;
    case "watch":
      return action.label.startsWith("Stop") ? EyeOffIcon : EyeIcon;
    case "history":
      return HistoryIcon;
    case "stop":
      return OctagonIcon;
    case "free-port":
      return OctagonXIcon;
    case "trash":
      return TrashIcon;
    case "delete":
      return Trash2Icon;
  }
}

interface RowMenuItemsProps {
  groups: RowAction[][];
  onRun: (id: RowActionId) => void;
  /** The item and separator of whichever menu this is drawn in. */
  Item: ComponentType<{
    variant?: "default" | "destructive";
    disabled?: boolean;
    onSelect?: () => void;
    className?: string;
    children?: ReactNode;
  }>;
  Separator: ComponentType;
}

/**
 * A row's actions, drawn the same in the "…" menu and the right-click menu.
 * An item that cannot be used stays in its place and says why.
 */
export function RowMenuItems({
  groups,
  onRun,
  Item,
  Separator,
}: RowMenuItemsProps) {
  // "Delete Permanently" takes the place of "Move to Trash" while a key is
  // held. Keyed as one item, it is the same element with new words, and the
  // keyboard focus on it stays where it is.
  const slot = (action: RowAction) =>
    action.id === "delete" ? "trash" : action.id;

  return groups.map((group, index) => (
    <Fragment key={slot(group[0])}>
      {index > 0 && <Separator />}
      {group.map((action) => {
        const Icon = iconFor(action);
        return (
          <Item
            key={slot(action)}
            variant={action.destructive ? "destructive" : "default"}
            disabled={!!action.disabledReason}
            onSelect={() => onRun(action.id)}
            // The reason has to stay readable, so only the label is dimmed.
            className="max-w-72 flex-col items-stretch gap-0.5 data-[disabled]:opacity-100"
          >
            <span
              className={
                action.disabledReason
                  ? "flex items-center gap-2 opacity-50"
                  : "flex items-center gap-2"
              }
            >
              <Icon />
              <span className="flex-1">{action.label}</span>
              {action.shortcut && (
                <kbd className="ml-6 font-sans text-xs tracking-wide text-muted-foreground">
                  {action.shortcut}
                </kbd>
              )}
            </span>
            {action.disabledReason && (
              <span className="pl-6 text-xs text-muted-foreground">
                {action.disabledReason}
              </span>
            )}
          </Item>
        );
      })}
    </Fragment>
  ));
}
