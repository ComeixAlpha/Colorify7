import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import zh from "./locales/zh.json";

export const resources = {
  zh: { translation: zh },
  en: { translation: en },
} as const;

export type AppLanguage = keyof typeof resources;

export function normalizeLanguage(value: unknown): AppLanguage {
  return value === "en" ? "en" : "zh";
}

export function initI18n(language: AppLanguage) {
  return i18n.use(initReactI18next).init({
    resources,
    lng: language,
    fallbackLng: "zh",
    interpolation: {
      escapeValue: false,
    },
  });
}

export default i18n;
