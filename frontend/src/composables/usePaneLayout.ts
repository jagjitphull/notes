import { computed, ref, watch } from "vue";

const SIDEBAR_MIN = 160;
const SIDEBAR_MAX = 400;
const SIDEBAR_DEFAULT = 220;
const LIST_MIN = 220;
const LIST_MAX = 480;
const LIST_DEFAULT = 300;

function persistedNumber(key: string, fallback: number): number {
  const raw = localStorage.getItem(key);
  const n = raw ? Number(raw) : NaN;
  return Number.isFinite(n) ? n : fallback;
}

function persistedBool(key: string): boolean {
  return localStorage.getItem(key) === "true";
}

/**
 * Sidebar/note-list widths and collapsed state, draggable-resize handling,
 * and the resulting grid-template-columns — all persisted across restarts.
 */
export function usePaneLayout() {
  const sidebarWidth = ref(persistedNumber("notes-sidebar-width", SIDEBAR_DEFAULT));
  const listWidth = ref(persistedNumber("notes-list-width", LIST_DEFAULT));
  const sidebarCollapsed = ref(persistedBool("notes-sidebar-collapsed"));
  const listCollapsed = ref(persistedBool("notes-list-collapsed"));
  // Deliberately not persisted - a momentary "hide the chrome while I
  // write" toggle, not a layout preference; reopening the app should show
  // the normal layout rather than surprise the user with hidden panes.
  const focusMode = ref(false);

  watch(sidebarWidth, (w) => localStorage.setItem("notes-sidebar-width", String(w)));
  watch(listWidth, (w) => localStorage.setItem("notes-list-width", String(w)));
  watch(sidebarCollapsed, (v) => localStorage.setItem("notes-sidebar-collapsed", String(v)));
  watch(listCollapsed, (v) => localStorage.setItem("notes-list-collapsed", String(v)));

  // What's actually shown right now - collapsed-by-the-user OR hidden by
  // focus mode, without focus mode touching (and overwriting on toggle
  // back off) the user's own persisted collapsed preference.
  const sidebarVisible = computed(() => !sidebarCollapsed.value && !focusMode.value);
  const listVisible = computed(() => !listCollapsed.value && !focusMode.value);

  function toggleFocusMode() {
    focusMode.value = !focusMode.value;
  }

  const gridTemplateColumns = computed(() => {
    const sidebar = sidebarVisible.value ? `${sidebarWidth.value}px` : "0px";
    const sidebarHandle = sidebarVisible.value ? "6px" : "0px";
    const list = listVisible.value ? `${listWidth.value}px` : "0px";
    const listHandle = listVisible.value ? "6px" : "0px";
    return `${sidebar} ${sidebarHandle} ${list} ${listHandle} 1fr`;
  });

  let resizing: "sidebar" | "list" | null = null;
  let startX = 0;
  let startWidth = 0;

  function onPointerMove(e: PointerEvent) {
    if (!resizing) return;
    const delta = e.clientX - startX;
    if (resizing === "sidebar") {
      sidebarWidth.value = Math.min(Math.max(startWidth + delta, SIDEBAR_MIN), SIDEBAR_MAX);
    } else {
      listWidth.value = Math.min(Math.max(startWidth + delta, LIST_MIN), LIST_MAX);
    }
  }

  function onPointerUp() {
    resizing = null;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  }

  function startResize(which: "sidebar" | "list", e: PointerEvent) {
    resizing = which;
    startX = e.clientX;
    startWidth = which === "sidebar" ? sidebarWidth.value : listWidth.value;
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    e.preventDefault();
  }

  // Keyboard equivalent of dragging the handle (Left/Right arrows), for
  // the resize handles' role="separator" - dragging alone would leave
  // pane resizing unreachable without a pointer.
  function stepResize(which: "sidebar" | "list", delta: number) {
    if (which === "sidebar") {
      sidebarWidth.value = Math.min(Math.max(sidebarWidth.value + delta, SIDEBAR_MIN), SIDEBAR_MAX);
    } else {
      listWidth.value = Math.min(Math.max(listWidth.value + delta, LIST_MIN), LIST_MAX);
    }
  }

  const bounds = {
    sidebar: { min: SIDEBAR_MIN, max: SIDEBAR_MAX },
    list: { min: LIST_MIN, max: LIST_MAX },
  };

  return {
    sidebarWidth,
    listWidth,
    sidebarCollapsed,
    listCollapsed,
    focusMode,
    toggleFocusMode,
    sidebarVisible,
    listVisible,
    gridTemplateColumns,
    startResize,
    stepResize,
    bounds,
  };
}
