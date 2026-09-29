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

  watch(sidebarWidth, (w) => localStorage.setItem("notes-sidebar-width", String(w)));
  watch(listWidth, (w) => localStorage.setItem("notes-list-width", String(w)));
  watch(sidebarCollapsed, (v) => localStorage.setItem("notes-sidebar-collapsed", String(v)));
  watch(listCollapsed, (v) => localStorage.setItem("notes-list-collapsed", String(v)));

  const gridTemplateColumns = computed(() => {
    const sidebar = sidebarCollapsed.value ? "0px" : `${sidebarWidth.value}px`;
    const sidebarHandle = sidebarCollapsed.value ? "0px" : "6px";
    const list = listCollapsed.value ? "0px" : `${listWidth.value}px`;
    const listHandle = listCollapsed.value ? "0px" : "6px";
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

  return {
    sidebarWidth,
    listWidth,
    sidebarCollapsed,
    listCollapsed,
    gridTemplateColumns,
    startResize,
  };
}
