import { useDeferredValue, useMemo, useState } from "react";
import { filterPortProcesses } from "@/lib/port-filter";
import { useSettings } from "@/lib/settings-store";
import { parsePort, processHasPort, type PortProcess } from "@/lib/types";

/**
 * The search box and what it selects. The list is filtered once, from the
 * deferred text, and that one result is what the table shows and what an
 * export copies.
 */
export function usePortQuery(processes: PortProcess[], loading: boolean) {
  const { hideSystemServices, hideUserServices, searchField } = useSettings();
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);

  const shown = useMemo(
    () =>
      filterPortProcesses(
        processes,
        hideSystemServices,
        hideUserServices,
        deferredSearch,
        searchField,
      ),
    [
      processes,
      hideSystemServices,
      hideUserServices,
      searchField,
      deferredSearch,
    ],
  );

  // A whole port number typed into a port search is a question about that
  // port: who holds it, or that nobody does.
  const exactPortQuery = searchField === "port" ? parsePort(search) : null;
  const portLookupOccupants = useMemo(
    () =>
      exactPortQuery === null
        ? []
        : processes.filter((process) =>
            processHasPort(process, exactPortQuery),
          ),
    [exactPortQuery, processes],
  );

  return {
    search,
    setSearch,
    shown,
    exactPortQuery,
    portLookupOccupants,
    portLookupEmpty:
      exactPortQuery !== null && !loading && portLookupOccupants.length === 0,
  };
}
