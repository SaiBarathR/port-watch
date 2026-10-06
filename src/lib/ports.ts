import type { PortBinding } from "@/lib/types";

/**
 * Who can reach a listening port: any machine that can reach this one, only
 * this machine, or machines on the one network a specific address is on.
 */
export type PortReach = "everyone" | "this-machine" | "one-address";

// Each platform spells "every interface" its own way.
const EVERY_INTERFACE = new Set(["*", "0.0.0.0", "::", "[::]"]);

export function reachOf(address: string): PortReach {
  // Without the brackets and zone of an IPv6 address ("[fe80::1%en0]"), and
  // as IPv4 when it is one written the IPv6 way ("::ffff:127.0.0.1").
  const plain = address
    .replace(/^\[|\]$/g, "")
    .replace(/%.*$/, "")
    .replace(/^::ffff:(?=\d+\.)/i, "");

  if (EVERY_INTERFACE.has(address) || EVERY_INTERFACE.has(plain)) {
    return "everyone";
  }
  if (plain.startsWith("127.") || plain === "::1" || plain === "localhost") {
    return "this-machine";
  }
  return "one-address";
}

/**
 * One port number a process holds, however many addresses it holds it on and
 * whether over TCP, UDP or both.
 */
export interface HeldPort {
  port: number;
  /** "TCP", "UDP", or both, TCP first. */
  protocols: string[];
  /** The widest reach among its addresses. */
  reach: PortReach;
  addresses: string[];
}

const WIDER: Record<PortReach, number> = {
  "this-machine": 0,
  "one-address": 1,
  everyone: 2,
};

/**
 * A process's ports, each number once. A server on 127.0.0.1 and [::1]
 * holds one port, not two, and so does a resolver on 53 over TCP and UDP.
 */
export function heldPorts(bindings: PortBinding[]): HeldPort[] {
  const held = new Map<number, HeldPort>();
  for (const binding of bindings) {
    const protocol = binding.protocol.toUpperCase();
    const reach = reachOf(binding.address);
    const existing = held.get(binding.port);
    if (!existing) {
      held.set(binding.port, {
        port: binding.port,
        protocols: [protocol],
        reach,
        addresses: [binding.address],
      });
      continue;
    }
    if (!existing.protocols.includes(protocol)) {
      existing.protocols.push(protocol);
      existing.protocols.sort();
    }
    if (!existing.addresses.includes(binding.address)) {
      existing.addresses.push(binding.address);
    }
    if (WIDER[reach] > WIDER[existing.reach]) {
      existing.reach = reach;
    }
  }
  return [...held.values()];
}

// Rough widths of what a port takes in a row, in px. The number is set in
// 15px monospace, where a digit is a little over 9px wide in every font the
// app falls back to; the tag is 10px text.
const DIGIT = 9.25;
const TAG_LETTER = 6.5;
const GLYPH = 16;
const GAP = 8;

/**
 * How many of a process's ports fit in `width` px, leaving room to say how
 * many do not. At least one: a row is about its port.
 */
export function portsThatFit(
  ports: HeldPort[],
  width: number,
  moreIndicator = 28,
): number {
  let used = 0;
  for (const [index, held] of ports.entries()) {
    const tag = protocolTag(held);
    const label =
      String(held.port).length * DIGIT +
      (tag ? tag.length * TAG_LETTER + 4 : 0) +
      GLYPH;
    const left = ports.length - index - 1;
    const needed = used + label + (left > 0 ? GAP + moreIndicator : 0);
    if (index > 0 && needed > width) {
      return index;
    }
    used += label + GAP;
  }
  return ports.length;
}

/** "udp" or "tcp+udp"; nothing for plain TCP, which is what a port usually is. */
export function protocolTag(held: HeldPort): string | null {
  return held.protocols.includes("UDP")
    ? held.protocols.join("+").toLowerCase()
    : null;
}

export function reachLabel(held: HeldPort): string {
  switch (held.reach) {
    case "everyone":
      return "Open to other machines: listening on every network interface";
    case "this-machine":
      return "This machine only";
    case "one-address":
      return `Open to other machines on ${held.addresses
        .filter((address) => reachOf(address) === "one-address")
        .join(", ")}`;
  }
}

/** A folder as a row shows it: its own name, and where it sits. */
export interface FolderLabel {
  name: string;
  /** The folder it is in, with the home folder shortened to "~". */
  parent: string;
}

/** Null for no folder at all and for the root, which says nothing. */
export function folderLabel(path: string): FolderLabel | null {
  const separator = path.includes("\\") && !path.includes("/") ? "\\" : "/";
  const trimmed = path.replace(/[\\/]+$/, "");
  if (trimmed === "" || /^[A-Za-z]:$/.test(trimmed)) {
    return null;
  }

  const cut = trimmed.lastIndexOf(separator);
  const name = trimmed.slice(cut + 1);
  const parent = cut <= 0 ? separator : trimmed.slice(0, cut);
  return { name, parent: shortenHome(parent) };
}

function shortenHome(path: string): string {
  return path
    .replace(/^\/(?:Users|home)\/[^/]+/, "~")
    .replace(/^[A-Za-z]:\\Users\\[^\\]+/, "~");
}
