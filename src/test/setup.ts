import { afterEach } from "vitest";

// Most tests are of plain functions and run without a DOM. The ones that
// draw components ask for jsdom at the top of their file, and get from here
// the parts of a browser jsdom leaves out and the components reach for.
if (typeof window !== "undefined") {
  class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  window.ResizeObserver ??= ResizeObserver;

  window.matchMedia ??= (query: string) =>
    ({
      matches: false,
      media: query,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
      dispatchEvent: () => false,
      onchange: null,
    }) as MediaQueryList;

  // Enough for the row ids it is used on.
  Object.assign(window, {
    CSS: {
      ...window.CSS,
      escape: (value: string) => value.replace(/[^\w-]/g, "\\$&"),
    },
  });

  // Menus scroll their first item into view and capture the pointer.
  Element.prototype.scrollIntoView ??= () => {};
  Element.prototype.hasPointerCapture ??= () => false;
  Element.prototype.setPointerCapture ??= () => {};
  Element.prototype.releasePointerCapture ??= () => {};

  const { cleanup, configure } = await import("@testing-library/react");
  // A CI runner under load takes its time over a dialog; the default second
  // is not always enough there.
  configure({ asyncUtilTimeout: 5000 });

  afterEach(() => {
    cleanup();
    localStorage.clear();
  });
}
