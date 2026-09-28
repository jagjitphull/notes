import { onMounted, onUnmounted, ref } from "vue";

export type ThemePreference = "system" | "light" | "dark";

const STORAGE_KEY = "notes-theme";
const preference = ref<ThemePreference>(readStoredPreference());
const systemPrefersDark = ref(
  window.matchMedia("(prefers-color-scheme: dark)").matches,
);

function readStoredPreference(): ThemePreference {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "light" || stored === "dark" || stored === "system"
    ? stored
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
    const order: ThemePreference[] = ["system", "light", "dark"];
    const next = order[(order.indexOf(preference.value) + 1) % order.length];
    setPreference(next);
  }

  return { preference, systemPrefersDark, setPreference, cyclePreference };
}
