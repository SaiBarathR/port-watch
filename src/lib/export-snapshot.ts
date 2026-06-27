import type { PortProcess } from "@/lib/types";
import { formatPorts, primaryPath, systemKindLabel } from "@/lib/types";

export function processesToJson(processes: PortProcess[]): string {
  return JSON.stringify(processes, null, 2);
}

export function processesToMarkdown(processes: PortProcess[]): string {
  if (processes.length === 0) {
    return "_No processes._";
  }

  const header =
    "| Port(s) | Process | PID | User | Type | Directory |\n| --- | --- | --- | --- | --- | --- |";

  const rows = processes.map((process) => {
    const cells = [
      formatPorts(process.ports, true),
      process.name,
      String(process.pid),
      process.user,
      systemKindLabel(process.system_kind),
      process.project_root || process.working_directory || primaryPath(process),
    ];
    return `| ${cells.map(escapeCell).join(" | ")} |`;
  });

  return [header, ...rows].join("\n");
}

function escapeCell(value: string): string {
  return value.replace(/\|/g, "\\|").replace(/\n/g, " ");
}
