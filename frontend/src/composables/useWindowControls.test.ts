import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, h } from "vue";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";

const { windowMock } = vi.hoisted(() => ({
  windowMock: {
    isMaximized: vi.fn(),
    onResized: vi.fn(),
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => windowMock }));

const { useWindowControls } = await import("./useWindowControls");

// isMaximized/the resize-listener subscription are module-level singletons
// (one real window, shared however many components call this), so each
// test unmounts its wrapper - the last consumer unsubscribing - to leave a
// clean slate for the next test rather than an instance-scoped teardown.
let wrapper: VueWrapper | null = null;

function mountWithControls() {
  let controls!: ReturnType<typeof useWindowControls>;
  wrapper = mount(
    defineComponent({
      setup() {
        controls = useWindowControls();
        return () => h("div");
      },
    }),
  );
  return controls;
}

describe("useWindowControls", () => {
  beforeEach(() => {
    windowMock.isMaximized.mockReset().mockResolvedValue(false);
    windowMock.onResized.mockReset().mockResolvedValue(() => {});
    windowMock.minimize.mockReset();
    windowMock.toggleMaximize.mockReset();
    windowMock.close.mockReset();
  });

  afterEach(() => {
    wrapper?.unmount();
    wrapper = null;
  });

  it("minimize/toggleMaximize/close delegate to the current window", () => {
    const controls = mountWithControls();

    controls.minimize();
    controls.toggleMaximize();
    controls.close();

    expect(windowMock.minimize).toHaveBeenCalled();
    expect(windowMock.toggleMaximize).toHaveBeenCalled();
    expect(windowMock.close).toHaveBeenCalled();
  });

  it("picks up the window's maximized state on mount", async () => {
    windowMock.isMaximized.mockResolvedValue(true);
    const controls = mountWithControls();

    await flushPromises();

    expect(controls.isMaximized.value).toBe(true);
  });

  it("updates isMaximized when the window resizes (e.g. maximized via double-click, not the button)", async () => {
    let resizeHandler: () => void = () => {};
    windowMock.onResized.mockImplementation(async (handler: () => void) => {
      resizeHandler = handler;
      return () => {};
    });
    windowMock.isMaximized.mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    const controls = mountWithControls();
    await flushPromises();
    expect(controls.isMaximized.value).toBe(false);

    resizeHandler();
    await flushPromises();

    expect(controls.isMaximized.value).toBe(true);
  });
});
