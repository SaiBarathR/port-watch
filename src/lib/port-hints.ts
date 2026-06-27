/** Common dev port labels (offline, static). */
export const PORT_HINTS: Record<number, string> = {
  80: "HTTP",
  443: "HTTPS",
  3000: "Dev server",
  3001: "Dev server",
  4000: "Dev server",
  4200: "Angular",
  4321: "Dev server",
  5000: "Flask / dev",
  5173: "Vite",
  5432: "PostgreSQL",
  5672: "RabbitMQ",
  6379: "Redis",
  8000: "Dev server",
  8080: "HTTP alt",
  8443: "HTTPS alt",
  9000: "Dev / PHP-FPM",
  27017: "MongoDB",
  3306: "MySQL",
};

export function portHint(port: number): string | undefined {
  return PORT_HINTS[port];
}

export function portHintsLabel(ports: { port: number }[]): string | undefined {
  const labels = [
    ...new Set(
      ports.map((p) => portHint(p.port)).filter((label): label is string => !!label),
    ),
  ];
  return labels.length > 0 ? labels.join(", ") : undefined;
}
