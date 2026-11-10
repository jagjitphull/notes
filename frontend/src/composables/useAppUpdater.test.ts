import { beforeEach, describe, expect, it, vi } from "vitest";

const { checkMock, relaunchMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  relaunchMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({ check: checkMock }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: relaunchMock }));

const { useAppUpdater } = await import("./useAppUpdater");

// available/version/installing/error are module-level singletons (one
// update-check result shared app-wide), so each test resets them via a
// fresh checkForUpdate() call rather than getting an isolated instance.
describe("useAppUpdater", () => {
  beforeEach(() => {
    checkMock.mockReset();
    relaunchMock.mockReset();
    const state = useAppUpdater();
    state.available.value = false;
    state.version.value = null;
    state.installing.value = false;
    state.error.value = null;
  });

  it("does nothing when no update is available", async () => {
    checkMock.mockResolvedValue(null);
    const { available, checkForUpdate } = useAppUpdater();

    await checkForUpdate();

    expect(available.value).toBe(false);
  });

  it("exposes the version when an update is available", async () => {
    checkMock.mockResolvedValue({ version: "0.2.0", downloadAndInstall: vi.fn() });
    const { available, version, checkForUpdate } = useAppUpdater();

    await checkForUpdate();

    expect(available.value).toBe(true);
    expect(version.value).toBe("0.2.0");
  });

  it("swallows a failed check instead of throwing", async () => {
    checkMock.mockRejectedValue(new Error("offline"));
    const { available, error, checkForUpdate } = useAppUpdater();

    await expect(checkForUpdate()).resolves.toBeUndefined();

    expect(available.value).toBe(false);
    expect(error.value).toBe("offline");
  });

  it("installUpdate downloads, installs, and relaunches", async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    checkMock.mockResolvedValue({ version: "0.2.0", downloadAndInstall });
    const { checkForUpdate, installUpdate, installing } = useAppUpdater();

    await checkForUpdate();
    const installPromise = installUpdate();
    expect(installing.value).toBe(true);
    await installPromise;

    expect(downloadAndInstall).toHaveBeenCalled();
    expect(relaunchMock).toHaveBeenCalled();
  });

  it("a failed install stops the installing state instead of hanging", async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error("disk full"));
    checkMock.mockResolvedValue({ version: "0.2.0", downloadAndInstall });
    const { checkForUpdate, installUpdate, installing, error } = useAppUpdater();

    await checkForUpdate();
    await installUpdate();

    expect(installing.value).toBe(false);
    expect(error.value).toBe("disk full");
    expect(relaunchMock).not.toHaveBeenCalled();
  });
});
