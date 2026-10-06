import { describe, expect, it } from "vitest";
import {
  folderLabel,
  heldPorts,
  portsThatFit,
  protocolTag,
  reachLabel,
  reachOf,
} from "@/lib/ports";
import type { PortBinding } from "@/lib/types";

const binding = (
  address: string,
  port: number,
  protocol = "TCP",
): PortBinding => ({ address, port, protocol });

describe("reachOf", () => {
  it("knows every platform's spelling of every interface", () => {
    for (const address of ["*", "0.0.0.0", "::", "[::]"]) {
      expect(reachOf(address), address).toBe("everyone");
    }
  });

  it("knows loopback in both families", () => {
    for (const address of ["127.0.0.1", "127.0.0.53", "[::1]", "::1"]) {
      expect(reachOf(address), address).toBe("this-machine");
    }
  });

  // An IPv4 address held through an IPv6 socket, and an address with a zone.
  it("reads IPv4 written the IPv6 way, and ignores a zone", () => {
    expect(reachOf("[::ffff:127.0.0.1]")).toBe("this-machine");
    expect(reachOf("::FFFF:127.0.0.1")).toBe("this-machine");
    expect(reachOf("[::ffff:0.0.0.0]")).toBe("everyone");
    expect(reachOf("[::ffff:192.168.1.20]")).toBe("one-address");
    expect(reachOf("[::1%lo0]")).toBe("this-machine");
  });

  it("treats any other address as one network", () => {
    for (const address of ["192.168.1.20", "10.0.0.5", "[fe80::1%en0]"]) {
      expect(reachOf(address), address).toBe("one-address");
    }
  });
});

describe("heldPorts", () => {
  it("lists a port once however many addresses hold it", () => {
    expect(
      heldPorts([
        binding("127.0.0.1", 3000),
        binding("[::1]", 3000),
        binding("127.0.0.1", 9229),
      ]),
    ).toEqual([
      {
        port: 3000,
        protocols: ["TCP"],
        reach: "this-machine",
        addresses: ["127.0.0.1", "[::1]"],
      },
      {
        port: 9229,
        protocols: ["TCP"],
        reach: "this-machine",
        addresses: ["127.0.0.1"],
      },
    ]);
  });

  it("takes the widest reach among a port's addresses", () => {
    const [both] = heldPorts([binding("127.0.0.1", 80), binding("*", 80)]);
    expect(both.reach).toBe("everyone");

    const [lan] = heldPorts([
      binding("127.0.0.1", 80),
      binding("192.168.1.20", 80),
    ]);
    expect(lan.reach).toBe("one-address");
  });

  it("lists a port held over TCP and UDP once, and says so", () => {
    const ports = heldPorts([
      binding("*", 53, "udp"),
      binding("*", 53, "TCP"),
      binding("*", 5353, "UDP"),
      binding("*", 80),
    ]);

    expect(ports.map((held) => held.port)).toEqual([53, 5353, 80]);
    expect(ports.map(protocolTag)).toEqual(["tcp+udp", "udp", null]);
  });
});

describe("portsThatFit", () => {
  const ports = (...numbers: number[]) =>
    heldPorts(numbers.map((port) => binding("*", port)));

  it("shows every port when there is room", () => {
    expect(portsThatFit(ports(3000, 9229), 144)).toBe(2);
    expect(portsThatFit(ports(80), 56)).toBe(1);
  });

  it("keeps room to say how many are left out", () => {
    // Two fit side by side, but not with "+1" after them.
    expect(portsThatFit(ports(3000, 9229, 9230), 120)).toBe(1);
    expect(portsThatFit(ports(3000, 9229, 9230), 150)).toBe(2);
    expect(portsThatFit(ports(3000, 9229, 9230), 400)).toBe(3);
  });

  it("counts the protocol tag", () => {
    const tagged = heldPorts([
      binding("*", 49152, "UDP"),
      binding("*", 49153, "UDP"),
    ]);
    expect(portsThatFit(tagged, 144)).toBe(1);
    expect(portsThatFit(tagged, 200)).toBe(2);
  });

  it("always shows the first port, however narrow the column", () => {
    expect(portsThatFit(ports(49152, 49153), 10)).toBe(1);
  });
});

describe("reachLabel", () => {
  it("names the address a port is open on", () => {
    const [held] = heldPorts([
      binding("127.0.0.1", 80),
      binding("192.168.1.20", 80),
    ]);
    expect(reachLabel(held)).toBe("Open to other machines on 192.168.1.20");
  });
});

describe("folderLabel", () => {
  it("splits a folder into its name and where it sits", () => {
    expect(folderLabel("/Users/dev/projects/api")).toEqual({
      name: "api",
      parent: "~/projects",
    });
    expect(folderLabel("/home/dev/api/")).toEqual({ name: "api", parent: "~" });
    expect(folderLabel("/srv/www/site")).toEqual({
      name: "site",
      parent: "/srv/www",
    });
    expect(folderLabel("/opt")).toEqual({ name: "opt", parent: "/" });
  });

  it("reads Windows paths", () => {
    expect(folderLabel("C:\\Users\\dev\\app")).toEqual({
      name: "app",
      parent: "~",
    });
    expect(folderLabel("D:\\work\\api")).toEqual({
      name: "api",
      parent: "D:\\work",
    });
  });

  it("has nothing to say about the root or no folder", () => {
    for (const path of ["", "/", "C:\\", "C:"]) {
      expect(folderLabel(path), path).toBeNull();
    }
  });
});
