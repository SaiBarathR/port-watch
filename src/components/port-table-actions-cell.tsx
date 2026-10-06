import { createContext, memo, useContext, type ReactNode } from "react";
import { MoreHorizontalIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { RowMenuItems } from "@/components/row-menu";
import { useRowActionRunner, useRowMenu } from "@/hooks/use-row-actions";
import { returnFocusToRow } from "@/lib/row-focus";
import { formatPorts, type PortProcess } from "@/lib/types";

interface OpenMenuContextValue {
  openMenuId: string | null;
  setOpenMenuId: (id: string | null) => void;
}

export const OpenMenuContext = createContext<OpenMenuContextValue | null>(null);

export function OpenMenuProvider({
  openMenuId,
  setOpenMenuId,
  children,
}: OpenMenuContextValue & { children: ReactNode }) {
  return (
    <OpenMenuContext.Provider value={{ openMenuId, setOpenMenuId }}>
      {children}
    </OpenMenuContext.Provider>
  );
}

// Every port, in words: a row only spells out as many as fit its column.
function MenuHeading({ process }: { process: PortProcess }) {
  const udp = process.ports.some((binding) => binding.protocol === "UDP");
  return (
    <p className="max-w-72 truncate px-2 py-1.5 text-xs text-muted-foreground">
      {process.name} · {process.ports.length === 1 ? "port" : "ports"}{" "}
      {formatPorts(process.ports, udp)}
    </p>
  );
}

interface RowMenuProps {
  process: PortProcess;
  portIsShared: boolean;
}

interface PortTableActionsCellProps extends RowMenuProps {
  /** Not read: a settings change must redraw a menu that is open. */
  settings: object;
}

export const PortTableActionsCell = memo(function PortTableActionsCell({
  process,
  portIsShared,
}: PortTableActionsCellProps) {
  const menu = useContext(OpenMenuContext);
  if (!menu) {
    return null;
  }

  const { openMenuId, setOpenMenuId } = menu;
  const isOpen = openMenuId === process.id;

  return (
    <DropdownMenu
      open={isOpen}
      onOpenChange={(open) => {
        setOpenMenuId(open ? process.id : null);
      }}
      modal={false}
    >
      <DropdownMenuTrigger asChild>
        {/* Reached from the row with the menu key, not by tabbing. */}
        <Button
          variant="ghost"
          size="icon"
          tabIndex={-1}
          aria-label={`Actions for ${process.name}`}
        >
          <MoreHorizontalIcon />
        </Button>
      </DropdownMenuTrigger>
      {isOpen && (
        <RowDropdownContent
          process={process}
          portIsShared={portIsShared}
          onClose={() => setOpenMenuId(null)}
        />
      )}
    </DropdownMenu>
  );
});

function RowDropdownContent({
  process,
  portIsShared,
  onClose,
}: RowMenuProps & { onClose: () => void }) {
  const groups = useRowMenu(process, portIsShared);
  const run = useRowActionRunner();

  return (
    <DropdownMenuContent
      align="end"
      onCloseAutoFocus={(event) => {
        event.preventDefault();
        returnFocusToRow(process.id);
      }}
    >
      <MenuHeading process={process} />
      <DropdownMenuSeparator />
      <RowMenuItems
        groups={groups}
        onRun={(id) => {
          onClose();
          run(id, process, portIsShared);
        }}
        Item={DropdownMenuItem}
        Separator={DropdownMenuSeparator}
      />
    </DropdownMenuContent>
  );
}

/** The same menu, for a right-click on the row. */
export function RowContextMenuContent({ process, portIsShared }: RowMenuProps) {
  const groups = useRowMenu(process, portIsShared);
  const run = useRowActionRunner();

  return (
    <ContextMenuContent
      onCloseAutoFocus={(event) => {
        event.preventDefault();
        returnFocusToRow(process.id);
      }}
    >
      <MenuHeading process={process} />
      <ContextMenuSeparator />
      <RowMenuItems
        groups={groups}
        onRun={(id) => run(id, process, portIsShared)}
        Item={ContextMenuItem}
        Separator={ContextMenuSeparator}
      />
    </ContextMenuContent>
  );
}
