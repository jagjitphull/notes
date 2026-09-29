import { createI18n } from "vue-i18n";
import en from "./locales/en.json";

// English is the only shipped locale today; adding another is just
// dropping in another locales/<code>.json and listing it in `messages`.
export const i18n = createI18n({
  legacy: false,
  locale: "en",
  fallbackLocale: "en",
  messages: { en },
});
