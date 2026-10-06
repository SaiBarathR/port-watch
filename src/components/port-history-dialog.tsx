import { useState } from "react";
import { Trash2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  PortHistoryList,
  PortHistoryTimeline,
} from "@/components/port-history-timeline";
import { clearPortHistory } from "@/lib/port-history";

interface PortHistoryDialogProps {
  /** One port's history, "all" for every port's, or null when closed. */
  show: number | "all" | null;
  onClose: () => void;
}

/**
 * What has come and gone on the ports, as the scans saw it. It used to be a
 * section of Settings, which it is not one of.
 */
export function PortHistoryDialog({ show, onClose }: PortHistoryDialogProps) {
  const [selectedPort, setSelectedPort] = useState<number | null>(null);
  // Bumped to draw the list again once the history has been cleared.
  const [cleared, setCleared] = useState(0);

  return (
    <Dialog open={show !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[min(80vh,640px)] max-w-md flex-col">
        <DialogHeader>
          <DialogTitle>
            {typeof show === "number" ? `Port ${show} history` : "Port history"}
          </DialogTitle>
          <DialogDescription>
            {typeof show === "number"
              ? "When this port was taken and freed, as the scans saw it."
              : "When each port was first and last seen. Pick one for its timeline."}
          </DialogDescription>
        </DialogHeader>

        {typeof show === "number" && <PortHistoryTimeline port={show} />}

        {show === "all" && (
          <>
            <div key={cleared} className="min-h-0 flex-1 overflow-y-auto">
              <PortHistoryList
                selectedPort={selectedPort}
                onSelectPort={(port) =>
                  setSelectedPort((current) => (current === port ? null : port))
                }
              />
            </div>
            <div className="flex justify-end">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => {
                  clearPortHistory();
                  setSelectedPort(null);
                  setCleared((count) => count + 1);
                }}
              >
                <Trash2Icon />
                Clear history
              </Button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
