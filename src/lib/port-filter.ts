import type { AppSettings, PortProcess, SearchField } from "@/lib/types";
import { parsePort, processHasPort } from "@/lib/types";

/** Whether the "show user / system listeners" choice lets a process through. */
export function isShownByScope(
  process: PortProcess,
  scope: Pick<AppSettings, "hideSystemServices" | "hideUserServices">,
): boolean {
  return process.is_system_service
    ? !scope.hideSystemServices
    : !scope.hideUserServices;
}

function buildSearchHaystack(process: PortProcess): string {
  return [
    process.name,
    String(process.pid),
    process.user,
    process.command_line,
    process.working_directory,
    process.project_root,
    process.executable_path,
    process.script_path ?? "",
    ...process.ports.map((p) => `${p.address}:${p.port}/${p.protocol}`),
    ...process.ports.map((p) => String(p.port)),
  ]
    .join(" ")
    .toLowerCase();
}

function matchesSearch(
  process: PortProcess,
  query: string,
  field: SearchField,
  searchHaystacks: Map<string, string>,
): boolean {
  const q = query.trim().toLowerCase();
  if (!q) {
    return true;
  }

  switch (field) {
    case "port": {
      const port = parsePort(query);
      if (port !== null) {
        return processHasPort(process, port);
      }
      return process.ports.some((binding) => String(binding.port).includes(q));
    }
    case "pid":
      return String(process.pid).includes(q);
    case "process":
      return process.name.toLowerCase().includes(q);
    case "user":
      return process.user.toLowerCase().includes(q);
    case "path": {
      const pathHaystack = [
        process.working_directory,
        process.project_root,
        process.executable_path,
        process.script_path ?? "",
      ]
        .join(" ")
        .toLowerCase();
      return pathHaystack.includes(q);
    }
    case "command":
      return process.command_line.toLowerCase().includes(q);
    case "all":
      return searchHaystacks.get(process.id)?.includes(q) ?? false;
  }
}

export function filterPortProcesses(
  processes: PortProcess[],
  hideSystemServices: boolean,
  hideUserServices: boolean,
  search: string,
  searchField: SearchField,
): PortProcess[] {
  const trimmedSearch = search.trim();
  const searchHaystacks =
    trimmedSearch && searchField === "all"
      ? new Map(
          processes.map((process) => [
            process.id,
            buildSearchHaystack(process),
          ]),
        )
      : new Map<string, string>();

  return processes.filter(
    (process) =>
      isShownByScope(process, { hideSystemServices, hideUserServices }) &&
      matchesSearch(process, search, searchField, searchHaystacks),
  );
}
