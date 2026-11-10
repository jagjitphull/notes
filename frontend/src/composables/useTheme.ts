import { onMounted, onUnmounted, ref } from "vue";

export type ThemePreference =
  | "system"
  | "light"
  | "dark"
  | "paper"
  | "slate"
  | "highContrast"
  | "harbor"
  | "glass";

const PREFERENCES: ThemePreference[] = [
  "system",
  "light",
  "dark",
  "paper",
  "slate",
  "highContrast",
  "harbor",
  "glass",
];

const STORAGE_KEY = "notes-theme";
const preference = ref<ThemePreference>(readStoredPreference());
const systemPrefersDark = ref(
  window.matchMedia("(prefers-color-scheme: dark)").matches,
);

function readStoredPreference(): ThemePreference {
  const stored = localStorage.getItem(STORAGE_KEY);
  return (PREFERENCES as string[]).includes(stored ?? "")
    ? (stored as ThemePreference)
    : "system";
}

function applyToDocument() {
  const root = document.documentElement;
  if (preference.value === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.setAttribute("data-theme", preference.value);
  }
}

applyToDocument();

export function useTheme() {
  onMounted(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => {
      systemPrefersDark.value = e.matches;
    };
    media.addEventListener("change", onChange);
    onUnmounted(() => media.removeEventListener("change", onChange));
  });

  function setPreference(next: ThemePreference) {
    preference.value = next;
    localStorage.setItem(STORAGE_KEY, next);
    applyToDocument();
  }

  function cyclePreference() {
    const next =
      PREFERENCES[(PREFERENCES.indexOf(preference.value) + 1) % PREFERENCES.length];
    setPreference(next);
  }

  return {
    preference,
    systemPrefersDark,
    setPreference,
    cyclePreference,
    options: PREFERENCES,
  };
}
