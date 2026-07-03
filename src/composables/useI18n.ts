// Lightweight i18n composable for pivotsearch.
//
// Why not vue-i18n? This app has ~40 strings. A reactive ref + a lookup map
// covers it with zero dependencies and a ~60-line surface. vue-i18n would add
// ~30KB and a Composition API layer for no gain at this scale.
//
// Locale resolution order on first launch:
//   1. localStorage['pivotsearch.locale']  (user's prior manual choice)
//   2. navigator.language                  (zh-* → zh-CN, otherwise en)
// After setLocale(), the choice is persisted and wins on subsequent launches.

import { ref, computed } from "vue";
import en from "../locales/en";
import zhCN from "../locales/zh-CN";

export type Locale = "en" | "zh-CN";

const STORAGE_KEY = "pivotsearch.locale";

type Messages = Record<string, string>;

const MESSAGES: Record<Locale, Messages> = {
  en: en as unknown as Messages,
  "zh-CN": zhCN as unknown as Messages,
};

function detectInitialLocale(): Locale {
  // 1. Prior explicit user choice.
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "en" || stored === "zh-CN") return stored;

  // 2. Follow the OS / browser language.
  const nav = navigator.language?.toLowerCase() ?? "en";
  return nav.startsWith("zh") ? "zh-CN" : "en";
}

// Single shared reactive locale so every component stays in sync.
const locale = ref<Locale>(detectInitialLocale());

/** Translate a key, substituting {placeholder} tokens from `params`. */
function t(key: string, params?: Record<string, string | number>): string {
  const dict = MESSAGES[locale.value] ?? MESSAGES.en;
  let s = dict[key] ?? MESSAGES.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.replace(new RegExp(`\\{${k}\\}`, "g"), String(v));
    }
  }
  return s;
}

/** Switch locale at runtime and persist the choice. */
function setLocale(lang: Locale): void {
  locale.value = lang;
  localStorage.setItem(STORAGE_KEY, lang);
}

/** Toggle between the two supported locales. */
function toggleLocale(): void {
  setLocale(locale.value === "en" ? "zh-CN" : "en");
}

export function useI18n() {
  return {
    locale: computed(() => locale.value),
    t,
    setLocale,
    toggleLocale,
  };
}
