// @vitest-environment jsdom
import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { launchApp, listener, systemService } from "@/test/app";

// The app as a whole, against a backend that answers from memory: what the
// window asks the backend to stop and delete, and what it takes to get there.

const table = () => within(screen.getByRole("table"));

async function openRowMenu(user: UserEvent, processName: string) {
  await user.click(
    await screen.findByRole("button", { name: `Actions for ${processName}` }),
  );
  return within(await screen.findByRole("menu"));
}

describe("at launch", () => {
  it("says it is scanning until the first scan answers, not that nothing is listening", async () => {
    const app = await launchApp({ holdFirstScan: true });

    expect(table().getByText("Scanning ports…")).toBeTruthy();
    expect(table().queryByText("No listening ports found.")).toBeNull();

    await app.finishFirstScan();

    expect(await table().findByText("No listening ports found.")).toBeTruthy();
    expect(table().queryByText("Scanning ports…")).toBeNull();
  });

  it("lists user processes and leaves system services out until asked", async () => {
    await launchApp({
      processes: [listener(4242, 3000), systemService(88, 5000, "rapportd")],
    });

    const row = await screen.findByRole("row", { name: /3000/ });
    expect(within(row).getByText("node")).toBeTruthy();
    expect(screen.queryByText("rapportd")).toBeNull();
  });
});

describe("a row's menu", () => {
  it("counts a port held on two addresses as one port", async () => {
    const app = await launchApp({
      processes: [
        listener(4242, 3000, {
          ports: [
            { address: "127.0.0.1", port: 3000, protocol: "TCP" },
            { address: "::1", port: 3000, protocol: "TCP" },
          ],
        }),
        listener(4343, 8080, {
          name: "caddy",
          ports: [
            { address: "*", port: 8080, protocol: "TCP" },
            { address: "*", port: 8443, protocol: "TCP" },
          ],
        }),
      ],
    });

    let menu = await openRowMenu(app.user, "node");
    expect(menu.getByText("node · port 3000")).toBeTruthy();
    await app.user.keyboard("{Escape}");

    menu = await openRowMenu(app.user, "caddy");
    expect(menu.getByText("caddy · ports 8080, 8443")).toBeTruthy();
  });
});

describe("stopping", () => {
  it("stops the process the row showed, after one confirmation", async () => {
    const app = await launchApp({ processes: [listener(4242, 3000)] });

    const menu = await openRowMenu(app.user, "node");
    await app.user.click(menu.getByRole("menuitem", { name: /^Stop…/ }));

    const dialog = within(await screen.findByRole("alertdialog"));
    expect(dialog.getByText("Stop node on port 3000?")).toBeTruthy();
    expect(app.callsTo("stop_process")).toEqual([]);

    await app.user.click(dialog.getByRole("button", { name: "Stop Process" }));

    // The name and start time go with the PID: the backend refuses if the
    // PID has passed to another process since the scan.
    await waitFor(() =>
      expect(app.callsTo("stop_process")).toEqual([
        { pid: 4242, expectedName: "node", expectedStartedAt: 1_790_000_000 },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(screen.queryByText("node")).toBeNull());
  });

  it("never includes a system service in “Stop all user processes shown”", async () => {
    // The hardest case: system services are both listed and unlocked.
    const app = await launchApp({
      settings: { hideSystemServices: false, allowSystemProcessActions: true },
      processes: [
        listener(4242, 3000),
        systemService(88, 5000, "rapportd"),
        listener(4343, 4000, { name: "vite" }),
      ],
    });
    await screen.findByText("rapportd");

    await app.user.click(screen.getByRole("button", { name: "More actions" }));
    await app.user.click(
      await screen.findByRole("menuitem", {
        name: "Stop all user processes shown…",
      }),
    );

    const dialog = within(await screen.findByRole("alertdialog"));
    expect(dialog.getByText("Stop all 2 user processes shown?")).toBeTruthy();
    expect(dialog.queryByText(/rapportd/)).toBeNull();

    await app.user.click(
      dialog.getByRole("button", { name: "Stop 2 Processes" }),
    );

    await waitFor(() => expect(app.callsTo("stop_process")).toHaveLength(2));
    expect(
      app
        .callsTo("stop_process")
        .map((call) => call.pid)
        .sort(),
    ).toEqual([4242, 4343]);
    expect(await screen.findByText("rapportd")).toBeTruthy();
  });

  it("stops only what the search left in the table", async () => {
    const app = await launchApp({
      processes: [listener(4242, 3000), listener(4343, 4000, { name: "vite" })],
    });
    await screen.findByText("vite");

    await app.user.type(
      screen.getByRole("textbox", { name: "Search listeners" }),
      "vite",
    );
    await waitFor(() => expect(table().queryByText("node")).toBeNull());

    await app.user.click(screen.getByRole("button", { name: "More actions" }));
    await app.user.click(
      await screen.findByRole("menuitem", {
        name: "Stop all user processes shown…",
      }),
    );
    const dialog = within(await screen.findByRole("alertdialog"));
    await app.user.click(dialog.getByRole("button", { name: "Stop Process" }));

    await waitFor(() =>
      expect(app.callsTo("stop_process").map((call) => call.pid)).toEqual([
        4343,
      ]),
    );
  });

  it("offers no stop for a system service while those are locked", async () => {
    const app = await launchApp({
      settings: { hideSystemServices: false },
      processes: [systemService(88, 5000, "rapportd")],
    });

    const menu = await openRowMenu(app.user, "rapportd");
    const stop = menu.getByRole("menuitem", { name: /^Stop…/ });
    expect(stop.getAttribute("aria-disabled")).toBe("true");

    await app.user.click(stop);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(app.callsTo("stop_process")).toEqual([]);
  });

  it("asks twice before stopping a system service that has been unlocked", async () => {
    const app = await launchApp({
      settings: { hideSystemServices: false, allowSystemProcessActions: true },
      processes: [systemService(88, 5000, "rapportd")],
    });

    const menu = await openRowMenu(app.user, "rapportd");
    await app.user.click(menu.getByRole("menuitem", { name: /^Stop…/ }));

    const dialog = within(await screen.findByRole("alertdialog"));
    expect(dialog.queryByRole("button", { name: "Stop Process" })).toBeNull();
    await app.user.click(dialog.getByRole("button", { name: "Continue" }));

    // The first press only brought up the warning.
    expect(dialog.getByText("System service warning")).toBeTruthy();
    expect(app.callsTo("stop_process")).toEqual([]);

    await app.user.click(dialog.getByRole("button", { name: "Stop Process" }));

    await waitFor(() =>
      expect(app.callsTo("stop_process").map((call) => call.pid)).toEqual([88]),
    );
  });

  it("starts the two confirmations over when the dialog is reopened", async () => {
    const app = await launchApp({
      settings: { hideSystemServices: false, allowSystemProcessActions: true },
      processes: [systemService(88, 5000, "rapportd")],
    });

    let menu = await openRowMenu(app.user, "rapportd");
    await app.user.click(menu.getByRole("menuitem", { name: /^Stop…/ }));
    let dialog = within(await screen.findByRole("alertdialog"));
    await app.user.click(dialog.getByRole("button", { name: "Continue" }));
    await app.user.click(dialog.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());

    menu = await openRowMenu(app.user, "rapportd");
    await app.user.click(menu.getByRole("menuitem", { name: /^Stop…/ }));
    dialog = within(await screen.findByRole("alertdialog"));

    expect(dialog.getByRole("button", { name: "Continue" })).toBeTruthy();
    expect(app.callsTo("stop_process")).toEqual([]);
  });

  it("scans again after a stop the backend refused, so a stale row does not stay", async () => {
    const app = await launchApp({
      processes: [listener(4242, 3000)],
      refuseToStop: { 4242: "PID 4242 now belongs to a different process" },
    });

    const menu = await openRowMenu(app.user, "node");
    await app.user.click(menu.getByRole("menuitem", { name: /^Stop…/ }));
    const dialog = within(await screen.findByRole("alertdialog"));
    expect(app.callsTo("trigger_port_scan")).toEqual([]);

    await app.user.click(dialog.getByRole("button", { name: "Stop Process" }));

    expect(
      await screen.findByText(/PID 4242 now belongs to a different process/),
    ).toBeTruthy();
    await waitFor(() =>
      expect(app.callsTo("trigger_port_scan")).toHaveLength(1),
    );
  });
});

describe("deleting a project folder", () => {
  it("moves it to the Trash without asking for the name", async () => {
    const app = await launchApp({ processes: [listener(4242, 3000)] });

    const menu = await openRowMenu(app.user, "node");
    await app.user.click(
      menu.getByRole("menuitem", { name: "Move to Trash…" }),
    );

    const dialog = within(await screen.findByRole("dialog"));
    expect(dialog.queryByRole("textbox")).toBeNull();
    expect(app.callsTo("delete_project")).toEqual([]);

    await app.user.click(dialog.getByRole("button", { name: "Move to Trash" }));

    await waitFor(() =>
      expect(app.callsTo("delete_project")).toEqual([
        {
          pid: 4242,
          expectedName: "node",
          expectedStartedAt: 1_790_000_000,
          path: "/Users/dev/app-3000",
          mode: "trash",
          confirmation: null,
        },
      ]),
    );
  });

  it("deletes permanently only once the folder's name has been typed", async () => {
    const app = await launchApp({ processes: [listener(4242, 3000)] });

    // Holding Option turns "Move to Trash" into "Delete Permanently".
    await app.user.keyboard("{Alt>}");
    const menu = await openRowMenu(app.user, "node");
    await app.user.click(
      menu.getByRole("menuitem", { name: "Delete Permanently…" }),
    );
    await app.user.keyboard("{/Alt}");

    const dialog = within(await screen.findByRole("dialog"));
    const confirm = dialog.getByRole("button", { name: "Delete Permanently" });
    const name = dialog.getByRole("textbox");
    expect(confirm).toHaveProperty("disabled", true);

    await app.user.type(name, "app-300");
    expect(confirm).toHaveProperty("disabled", true);
    await app.user.click(confirm);
    expect(app.callsTo("delete_project")).toEqual([]);

    await app.user.type(name, "0");
    expect(confirm).toHaveProperty("disabled", false);
    await app.user.click(confirm);

    await waitFor(() =>
      expect(app.callsTo("delete_project")).toEqual([
        {
          pid: 4242,
          expectedName: "node",
          expectedStartedAt: 1_790_000_000,
          path: "/Users/dev/app-3000",
          mode: "permanent",
          confirmation: "app-3000",
        },
      ]),
    );
  });

  it("does not offer it for a folder the backend will not delete", async () => {
    const app = await launchApp({
      processes: [
        listener(4242, 3000, {
          delete_blocked: "This folder is your home folder.",
        }),
      ],
    });

    const menu = await openRowMenu(app.user, "node");
    const trash = menu.getByRole("menuitem", { name: /Move to Trash…/ });
    expect(trash.getAttribute("aria-disabled")).toBe("true");

    await app.user.click(trash);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("scans again after a delete that failed: the process may have been stopped", async () => {
    const app = await launchApp({
      processes: [listener(4242, 3000)],
      refuseToDelete: "The folder changed while the process was stopping",
    });

    const menu = await openRowMenu(app.user, "node");
    await app.user.click(
      menu.getByRole("menuitem", { name: "Move to Trash…" }),
    );
    const dialog = within(await screen.findByRole("dialog"));
    await app.user.click(dialog.getByRole("button", { name: "Move to Trash" }));

    expect(
      await screen.findByText(/The folder changed while the process/),
    ).toBeTruthy();
    await waitFor(() =>
      expect(app.callsTo("trigger_port_scan")).toHaveLength(1),
    );
  });
});

describe("from the keyboard", () => {
  it("reaches the same confirmation as the menu", async () => {
    const app = await launchApp({ processes: [listener(4242, 3000)] });

    (await screen.findByRole("row", { name: /3000/ })).focus();
    await app.user.keyboard("{Meta>}{Backspace}{/Meta}");

    const dialog = within(await screen.findByRole("alertdialog"));
    expect(dialog.getByText("Stop node on port 3000?")).toBeTruthy();
    expect(app.callsTo("stop_process")).toEqual([]);
  });

  it("does not get past the lock on system services", async () => {
    const app = await launchApp({
      settings: { hideSystemServices: false },
      processes: [systemService(88, 5000, "rapportd")],
    });

    (await screen.findByRole("row", { name: /5000/ })).focus();
    await app.user.keyboard("{Meta>}{Backspace}{/Meta}");

    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(app.callsTo("stop_process")).toEqual([]);
  });
});

describe("settings", () => {
  async function openSettings(user: UserEvent) {
    await user.click(screen.getByRole("button", { name: "Settings" }));
    return within(await screen.findByRole("dialog"));
  }

  it("unlock system services only when that switch is turned on", async () => {
    const app = await launchApp({
      settings: { hideSystemServices: false },
      processes: [systemService(88, 5000, "rapportd")],
    });
    await screen.findByText("rapportd");

    const settings = await openSettings(app.user);
    const unlock = settings.getByRole("switch", {
      name: "Allow actions on system services",
    });
    expect(unlock.getAttribute("aria-checked")).toBe("false");
    await app.user.click(unlock);

    await waitFor(() =>
      expect(app.callsTo("update_settings")).toEqual([
        { patch: { allowSystemProcessActions: true } },
      ]),
    );
    await app.user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    const menu = await openRowMenu(app.user, "rapportd");
    expect(
      menu
        .getByRole("menuitem", { name: /^Stop…/ })
        .getAttribute("aria-disabled"),
    ).toBeNull();
  });

  it("go back to what they were when the backend does not save a change", async () => {
    const app = await launchApp({ refuseSettings: "The disk is full" });

    const settings = await openSettings(app.user);
    const unlock = settings.getByRole("switch", {
      name: "Allow actions on system services",
    });
    await app.user.click(unlock);

    expect(await screen.findByText("Could not save settings")).toBeTruthy();
    await waitFor(() =>
      expect(unlock.getAttribute("aria-checked")).toBe("false"),
    );
  });

  it("take only a port number as a port to watch", async () => {
    const app = await launchApp();

    const settings = await openSettings(app.user);
    const port = settings.getByRole("textbox", { name: "Watched ports" });
    const add = settings.getByRole("button", { name: "Add" });

    await app.user.type(port, "70000");
    await app.user.click(add);
    await app.user.clear(port);
    await app.user.type(port, "0");
    await app.user.click(add);
    expect(app.callsTo("update_settings")).toEqual([]);

    await app.user.clear(port);
    await app.user.type(port, "30a0.0{Enter}");

    await waitFor(() =>
      expect(app.callsTo("update_settings")).toEqual([
        { patch: { watchedPorts: [3000] } },
      ]),
    );
    expect(
      await settings.findByRole("button", { name: "Stop watching port 3000" }),
    ).toBeTruthy();
  });
});

describe("port change toasts", () => {
  it("announces a new listener", async () => {
    const app = await launchApp({ processes: [listener(4242, 3000)] });
    await screen.findByText("node");

    await app.scanFinds([
      listener(4242, 3000),
      listener(4343, 4000, { name: "vite" }),
    ]);

    expect(await screen.findByText("Port change detected")).toBeTruthy();
  });

  it("takes the open ones away when they are muted, and shows no more", async () => {
    const app = await launchApp({ processes: [listener(4242, 3000)] });
    await screen.findByText("node");
    await app.scanFinds([
      listener(4242, 3000),
      listener(4343, 4000, { name: "vite" }),
    ]);
    await screen.findByText("Port change detected");

    await app.user.click(
      screen.getByRole("button", {
        name: "Mute port change toasts for 15 minutes",
      }),
    );

    await waitFor(() =>
      expect(screen.queryByText("Port change detected")).toBeNull(),
    );
    expect(await screen.findByText("Port change toasts muted")).toBeTruthy();
    const [{ patch }] = app.callsTo("update_settings");
    expect(patch).toHaveProperty("changeToastsMutedUntil");

    await app.scanFinds([listener(4242, 3000)]);
    await app.scanFinds([listener(4242, 3000), listener(4444, 5173)]);

    expect(await table().findByText("5173")).toBeTruthy();
    expect(screen.queryByText(/port changes? detected/i)).toBeNull();
  });
});
