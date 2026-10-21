import { ref } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const available = ref(false);
const version = ref<string | null>(null);
const installing = ref(false);
const error = ref<string | null>(null);

let pendingUpdate: Update | null = null;

/**
 * Checks the configured update endpoint (see tauri.conf.json) once. Meant
 * to be called on app startup; failures (offline, no release published
 * yet, endpoint unreachable) are swallowed rather than surfaced - an
 * update check is a nice-to-have, not something that should interrupt
 * using the app.
 */
async function checkForUpdate() {
  try {
    const update = await check();
    if (update) {
      pendingUpdate = update;
      version.value = update.version;
      available.value = true;
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

/** Downloads, installs, and relaunches into the new version. */
async function installUpdate() {
  if (!pendingUpdate || installing.value) return;
  installing.value = true;
  try {
    await pendingUpdate.downloadAndInstall();
    await relaunch();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    installing.value = false;
  }
}

export function useAppUpdater() {
  return { available, version, installing, error, checkForUpdate, installUpdate };
}
