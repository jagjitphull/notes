import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";

const isMaximized = ref(false);

let unlistenResize: UnlistenFn | null = null;
let listenerCount = 0;

async function refreshMaximized() {
  isMaximized.value = await getCurrentWindow().isMaximized();
}

/**
 * Native window decorations are off (see tauri.conf.json) - App.vue draws
 * its own title bar and calls these instead. `isMaximized` tracks live
 * state so the maximize button can swap to a "restore" icon, including
 * when the window is maximized/restored by means other than that button
 * (double-clicking the title bar, a window-manager keyboard shortcut).
 */
export function useWindowControls() {
  onMounted(async () => {
    listenerCount++;
    await refreshMaximized();
    if (!unlistenResize) {
      unlistenResize = await getCurrentWindow().onResized(refreshMaximized);
    }
  });

  onUnmounted(() => {
    listenerCount--;
    if (listenerCount === 0) {
      unlistenResize?.();
      unlistenResize = null;
    }
  });

  return {
    isMaximized,
    minimize: () => getCurrentWindow().minimize(),
    toggleMaximize: () => getCurrentWindow().toggleMaximize(),
    close: () => getCurrentWindow().close(),
  };
}
