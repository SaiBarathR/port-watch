import type { PortProcess, SearchField } from "@/lib/types";
import { processHasPort } from "@/lib/types";

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
  searchHaystacks: Map<number, string>,
): boolean {
  const q = query.trim().toLowerCase();
  if (!q) {
    return true;
  }

  switch (field) {
    case "port": {
      const portNum = Number.parseInt(query.trim(), 10);
      if (
        Number.isInteger(portNum) &&
        portNum >= 1 &&
        portNum <= 65535 &&
        String(portNum) === query.trim()
      ) {
        return processHasPort(process, portNum);
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
      return searchHaystacks.get(process.pid)?.includes(q) ?? false;
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
          processes.map((process) => [process.pid, buildSearchHaystack(process)]),
        )
      : new Map<number, string>();

  const filtered = processes.filter((process) => {
    if (hideSystemServices && process.is_system_service) {
      return false;
    }

    if (hideUserServices && !process.is_system_service) {
      return false;
    }

    return matchesSearch(process, search, searchField, searchHaystacks);
  });

  if (
    filtered.length === 0 &&
    processes.length > 0 &&
    !trimmedSearch &&
    hideSystemServices &&
    !hideUserServices
  ) {
    const userProcesses = processes.filter((process) => !process.is_system_service);
    if (userProcesses.length > 0) {
      return userProcesses;
    }
  }

  return filtered;
}

export function normalizePortProcess(
  raw: PortProcess & { isSystemService?: boolean },
): PortProcess {
  return {
    ...raw,
    ports: Array.isArray(raw.ports) ? raw.ports : [],
    is_system_service:
      raw.is_system_service === true || raw.isSystemService === true,
  };
}
