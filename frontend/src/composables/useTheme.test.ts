import { beforeEach, describe, expect, it } from "vitest";
import { useTheme } from "./useTheme";

// preference/systemPrefersDark are module-level singletons (shared across
// every useTheme() call, matching every real component in the app reading
// the same "current theme"), so each test resets them explicitly rather
// than getting a fresh instance.
describe("useTheme", () => {
  beforeEach(() => {
    localStorage.clear();
    useTheme().setPreference("system");
  });

  it("defaults to system with no data-theme attribute set", () => {
    expect(useTheme().preference.value).toBe("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });

  it("cyclePreference goes system -> light -> dark -> system", () => {
    const { preference, cyclePreference } = useTheme();
    cyclePreference();
    expect(preference.value).toBe("light");
    cyclePreference();
    expect(preference.value).toBe("dark");
    cyclePreference();
    expect(preference.value).toBe("system");
  });

  it("setPreference('dark') sets data-theme and persists it", () => {
    useTheme().setPreference("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem("notes-theme")).toBe("dark");
  });

  it("setPreference('system') removes the data-theme attribute", () => {
    const { setPreference } = useTheme();
    setPreference("dark");
    setPreference("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
    expect(localStorage.getItem("notes-theme")).toBe("system");
  });
});
