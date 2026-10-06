// One pass through the real thing: the built app in its own webview, a real
// listener, a real stop. Everything else is tested in pieces; this is the
// only place the pieces are seen working together.
//
//   node e2e/smoke.mjs <path to the built app> [tauri-driver arguments]
//
// It needs `tauri-driver` on the PATH, and the platform's WebDriver server
// (WebKitWebDriver on Linux, msedgedriver on Windows). macOS has no driver
// for WKWebView, so this does not run there.
//
// It talks to the driver over plain HTTP, so it needs no packages. With
// E2E_SCREENSHOTS set to a folder, it saves a picture of the window there at
// each step, and one named `failure.png` if a step fails.

import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";

const [application, ...driverArguments] = process.argv.slice(2);
if (!application) {
  console.error("usage: node e2e/smoke.mjs <application> [driver arguments]");
  process.exit(2);
}

// tauri-driver listens on 4444 unless one of its arguments says otherwise.
function driverPort() {
  for (const [index, argument] of driverArguments.entries()) {
    if (argument === "--port") {
      return driverArguments[index + 1];
    }
    if (argument.startsWith("--port=")) {
      return argument.slice("--port=".length);
    }
  }
  return "4444";
}
const DRIVER = `http://127.0.0.1:${driverPort()}`;

const screenshots = process.env.E2E_SCREENSHOTS;
const step = (text) => console.log(`- ${text}`);

async function webdriver(method, route, body) {
  const response = await fetch(`${DRIVER}${route}`, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const { value } = await response.json();
  if (!response.ok) {
    throw new Error(
      `${method} ${route}: ${value?.error ?? response.status}: ${value?.message ?? ""}`,
    );
  }
  return value;
}

async function until(what, check, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  let last;
  for (;;) {
    try {
      const found = await check();
      if (found) {
        return found;
      }
    } catch (error) {
      last = error;
    }
    if (Date.now() > deadline) {
      throw new Error(
        `Timed out waiting for ${what}${last ? ` (${last.message})` : ""}`,
      );
    }
    await sleep(250);
  }
}

// On Windows the driver's own children (the native driver and the app) would
// outlive it, so the whole tree is ended there.
function end(child) {
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      stdio: "ignore",
    });
  } else {
    child.kill();
  }
}

// A process the app will list as the user's own on every platform: its
// script is in the home folder. It stands in for a dev server.
function startListener(folder) {
  const script = join(folder, "listener.cjs");
  writeFileSync(
    script,
    `require("node:http")
      .createServer((request, response) => response.end("ok"))
      .listen(0, "127.0.0.1", function () {
        console.log(this.address().port);
      });`,
  );
  const child = spawn(process.execPath, [script], {
    cwd: folder,
    stdio: ["ignore", "pipe", "inherit"],
  });
  const exited = new Promise((done) => child.once("exit", done));
  const port = new Promise((done, failed) => {
    child.stdout.once("data", (data) => done(Number(String(data).trim())));
    child.once("error", failed);
  });
  return { child, exited, port };
}

async function main() {
  const folder = mkdtempSync(join(homedir(), "port-watch-e2e-"));
  const listener = startListener(folder);
  // The app's own settings go to a folder of their own where the platform
  // allows it, so a run on a developer's machine leaves theirs alone.
  const driver = spawn("tauri-driver", driverArguments, {
    stdio: "inherit",
    env: {
      ...process.env,
      XDG_CONFIG_HOME: join(folder, "config"),
      XDG_DATA_HOME: join(folder, "data"),
    },
  });
  let session;
  let failed = async () => {};

  try {
    const port = await listener.port;
    step(`a listener is up on port ${port} (PID ${listener.child.pid})`);

    await until("tauri-driver", () =>
      fetch(`${DRIVER}/status`).then((response) => response.ok),
    );
    ({ sessionId: session } = await webdriver("POST", "/session", {
      capabilities: {
        alwaysMatch: {
          "tauri:options": { application: resolve(application) },
        },
      },
    }));
    step("the app is running");

    const picture = async (name) => {
      if (!screenshots) {
        return;
      }
      mkdirSync(screenshots, { recursive: true });
      const png = await webdriver("GET", `/session/${session}/screenshot`);
      writeFileSync(join(screenshots, `${name}.png`), png, "base64");
    };
    failed = () => picture("failure");

    const find = (xpath) =>
      webdriver("POST", `/session/${session}/elements`, {
        using: "xpath",
        value: xpath,
      });
    const click = async (what, xpath) => {
      const [element] = await until(what, async () => {
        const found = await find(xpath);
        return found.length > 0 && found;
      });
      await webdriver(
        "POST",
        `/session/${session}/element/${element[ELEMENT]}/click`,
        {},
      );
    };

    // The port cell holds the number alone, so 3000 does not match 30000.
    const row = `//tr[.//*[normalize-space(text())="${port}"]]`;
    await until(`a row for port ${port}`, async () => {
      const found = await find(row);
      return found.length === 1;
    });
    step("the first scan listed it");
    await picture("1-listed");

    await click(
      "the row's actions button",
      `${row}//button[starts-with(@aria-label, "Actions for")]`,
    );
    await picture("2-menu");
    await click(
      "Stop… in the row's menu",
      `//*[@role="menuitem"][starts-with(normalize-space(.), "Stop…")]`,
    );
    await picture("3-confirm");
    await click(
      "the confirmation",
      `//*[@role="alertdialog"]//button[normalize-space(.)="Stop Process"]`,
    );
    step("asked the app to stop it, and confirmed");

    await Promise.race([
      listener.exited,
      sleep(15_000).then(() => {
        throw new Error("The listener is still running 15 s after the stop");
      }),
    ]);
    step("the listener has exited");

    await until(`the row for port ${port} to go`, async () => {
      const found = await find(row);
      return found.length === 0;
    });
    step("its row is gone");
    await picture("4-stopped");
  } catch (error) {
    await failed().catch(() => {});
    throw error;
  } finally {
    if (session) {
      await webdriver("DELETE", `/session/${session}`).catch(() => {});
    }
    end(driver);
    end(listener.child);
    await Promise.race([listener.exited, sleep(5_000)]);
    try {
      rmSync(folder, {
        recursive: true,
        force: true,
        maxRetries: 20,
        retryDelay: 250,
      });
    } catch {
      // A folder left behind is not a failed test, and must not hide one.
    }
  }
}

main().then(
  () => {
    console.log("ok");
    process.exit(0);
  },
  (error) => {
    console.error(`FAILED: ${error.message}`);
    process.exit(1);
  },
);
