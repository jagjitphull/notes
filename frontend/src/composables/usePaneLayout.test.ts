import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { usePaneLayout } from "./usePaneLayout";

describe("usePaneLayout", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("defaults to expanded panes at their default widths", () => {
    const { sidebarCollapsed, listCollapsed, gridTemplateColumns } = usePaneLayout();
    expect(sidebarCollapsed.value).toBe(false);
    expect(listCollapsed.value).toBe(false);
    expect(gridTemplateColumns.value).toBe("220px 6px 300px 6px 1fr");
  });

  it("collapsing a pane zeroes both its column and its resize handle", () => {
    const { sidebarCollapsed, listCollapsed, gridTemplateColumns } = usePaneLayout();
    sidebarCollapsed.value = true;
    expect(gridTemplateColumns.value).toBe("0px 0px 300px 6px 1fr");

    listCollapsed.value = true;
    expect(gridTemplateColumns.value).toBe("0px 0px 0px 0px 1fr");
  });

  it("persists collapsed state so a fresh instance picks it up", async () => {
    const first = usePaneLayout();
    first.sidebarCollapsed.value = true;
    await nextTick(); // the persisting watch() callback is async

    expect(localStorage.getItem("notes-sidebar-collapsed")).toBe("true");

    const second = usePaneLayout();
    expect(second.sidebarCollapsed.value).toBe(true);
  });

  it("dragging the sidebar handle resizes within its min/max bounds", async () => {
    const { sidebarWidth, startResize } = usePaneLayout();
    startResize("sidebar", { clientX: 100, preventDefault: () => {} } as unknown as PointerEvent);

    window.dispatchEvent(new MouseEvent("pointermove", { clientX: 150 }));
    expect(sidebarWidth.value).toBe(270); // default 220 + 50 delta

    // Push far past SIDEBAR_MAX (400) - should clamp, not run away.
    window.dispatchEvent(new MouseEvent("pointermove", { clientX: 2000 }));
    expect(sidebarWidth.value).toBe(400);

    window.dispatchEvent(new MouseEvent("pointerup"));
    // A move after pointerup should be ignored (listener removed).
    window.dispatchEvent(new MouseEvent("pointermove", { clientX: 100 }));
    expect(sidebarWidth.value).toBe(400);

    await nextTick();
    expect(localStorage.getItem("notes-sidebar-width")).toBe("400");
  });

  it("dragging the list handle only ever affects the list width", () => {
    const { sidebarWidth, listWidth, startResize } = usePaneLayout();
    startResize("list", { clientX: 0, preventDefault: () => {} } as unknown as PointerEvent);
    window.dispatchEvent(new MouseEvent("pointermove", { clientX: 40 }));

    expect(listWidth.value).toBe(340); // default 300 + 40 delta
    expect(sidebarWidth.value).toBe(220); // untouched
  });
});
