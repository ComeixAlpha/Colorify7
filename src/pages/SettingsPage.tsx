import {
  M3eFormField,
  M3eHeading,
  M3eIcon,
  M3eOption,
  M3eSelect,
  M3eSwitch,
} from "@m3e/react/all";
import { useState, type ChangeEvent } from "react";
import { useTranslation } from "react-i18next";
import i18n, { AppLanguage, normalizeLanguage } from "../i18n";
import { Storer } from "../stores/storer";

export interface AppSettings {
  language: AppLanguage;
  seedColor: string;
  autoSliceMcfunction: boolean;
  previewImage: boolean;
  webSocketPort: number;
}

export const DEFAULT_SETTINGS: AppSettings = {
  language: "zh",
  seedColor: "#a8c7fa",
  autoSliceMcfunction: true,
  previewImage: true,
  webSocketPort: 8080,
};

interface SettingsPageProps {
  seedColor: string;
  onSeedColorChange: (color: string) => void;
}

export default function SettingsPage({
  seedColor,
  onSeedColorChange,
}: SettingsPageProps) {
  const { t } = useTranslation();
  const language = i18n.language === "en" ? "en" : "zh";

  const [autoSliceMcfunction, setAutoSliceMcfunction] = useState(
    Storer.load<AppSettings>("settings").autoSliceMcfunction,
  );
  const [previewImage, setPreviewImage] = useState(
    Storer.load<AppSettings>("settings").previewImage,
  );
  const [webSocketPort, setWebSocketPort] = useState(
    Storer.load<AppSettings>("settings").webSocketPort,
  );

  const handleLanguageChange = (e: Event) => {
    const lang = normalizeLanguage((e.target as any)?.value);
    i18n.changeLanguage(lang);
    Storer.savePartial("settings", { language: lang });
  };

  const handleSeedColorChange = (e: ChangeEvent<HTMLInputElement>) => {
    onSeedColorChange(e.target.value);
  };

  const handlePreviewImageChange = (e: Event) => {
    const checked = (e.target as any)?.checked === true;
    setPreviewImage(checked);
    Storer.savePartial("settings", { previewImage: checked });
  };

  const handleAutoSliceMcfunctionChange = (e: Event) => {
    const checked = (e.target as any)?.checked === true;
    setAutoSliceMcfunction(checked);
    Storer.savePartial("settings", { autoSliceMcfunction: checked });
  };

  const handleWebSocketPortChange = (e: ChangeEvent<HTMLInputElement>) => {
    const parsed = Number(e.target.value);
    if (Number.isNaN(parsed)) return;
    const port = Math.min(65535, Math.max(1, Math.trunc(parsed)));
    setWebSocketPort(port);
    Storer.savePartial("settings", { webSocketPort: port });
  };

  return (
    <div className="w-full h-full overflow-y-auto p-6 flex flex-col gap-4">
      <M3eHeading variant="headline" size="small">
        {t("settings.title")}
      </M3eHeading>

      {/* 通用设置分组 */}
      <div className="flex flex-col gap-6">
        <M3eHeading
          variant="label"
          size="large"
          className="px-1 text-md-on-surface-variant"
        >
          {t("settings.general")}
        </M3eHeading>

        {/* 语言 */}
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <M3eIcon
              name="language"
              className="text-md-on-surface-variant shrink-0"
            />
            <div className="flex flex-col min-w-0">
              <span className="font-medium">{t("settings.languageLabel")}</span>
              <span className="text-xs text-md-on-surface-variant">
                {t("settings.languageDesc")}
              </span>
            </div>
          </div>

          <M3eFormField className="w-40 shrink-0" hideSubscript="always">
            <label slot="label" htmlFor="language-select">
              {t("settings.languageSelectLabel")}
            </label>
            <M3eSelect id="language-select" onChange={handleLanguageChange}>
              <M3eOption value="zh" selected={language === "zh"}>
                中文
              </M3eOption>
              <M3eOption value="en" selected={language === "en"}>
                English
              </M3eOption>
            </M3eSelect>
          </M3eFormField>
        </div>

        {/* 主题色 */}
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <M3eIcon
              name="palette"
              className="text-md-on-surface-variant shrink-0"
            />
            <div className="flex flex-col min-w-0">
              <span className="font-medium">
                {t("settings.themeColorLabel")}
              </span>
              <span className="text-xs text-md-on-surface-variant">
                {t("settings.themeColorDesc")}
              </span>
            </div>
          </div>

          <input
            type="color"
            value={seedColor}
            onChange={handleSeedColorChange}
            aria-label={t("settings.themeColorLabel")}
            className="w-12 h-9 shrink-0 rounded-md bg-transparent cursor-pointer"
          />
        </div>

        {/* 自动切分 mcfunction */}
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <M3eIcon
              name="data_object"
              className="text-md-on-surface-variant shrink-0"
            />
            <div className="flex flex-col min-w-0">
              <span className="font-medium">
                {t("settings.autoSliceMcfunctionLabel")}
              </span>
              <span className="text-xs text-md-on-surface-variant">
                {t("settings.autoSliceMcfunctionDesc")}
              </span>
            </div>
          </div>

          <M3eSwitch
            checked={autoSliceMcfunction}
            onChange={handleAutoSliceMcfunctionChange}
            aria-label={t("settings.autoSliceMcfunctionLabel")}
            className="shrink-0"
          />
        </div>

        {/* 输出预览图片 */}
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <M3eIcon
              name="data_object"
              className="text-md-on-surface-variant shrink-0"
            />
            <div className="flex flex-col min-w-0">
              <span className="font-medium">
                {t("settings.previewImageLabel")}
              </span>
              <span className="text-xs text-md-on-surface-variant">
                {t("settings.previewImageDesc")}
              </span>
            </div>
          </div>

          <M3eSwitch
            checked={previewImage}
            onChange={handlePreviewImageChange}
            aria-label={t("settings.previewImageLabel")}
            className="shrink-0"
          />
        </div>
      </div>

      <div className="flex flex-col gap-6 pt-6">
        <M3eHeading
          variant="label"
          size="large"
          className="px-1 text-md-on-surface-variant"
        >
          {t("settings.websocket")}
        </M3eHeading>

        {/* WebSocket 端口 */}
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <M3eIcon
              name="dns"
              className="text-md-on-surface-variant shrink-0"
            />
            <div className="flex flex-col min-w-0">
              <span className="font-medium">
                {t("settings.webSocketPortLabel")}
              </span>
              <span className="text-xs text-md-on-surface-variant">
                {t("settings.webSocketPortDesc")}
              </span>
            </div>
          </div>

          <M3eFormField className="w-40 shrink-0" hideSubscript="always">
            <label slot="label" htmlFor="ws-port">
              {t("settings.webSocketPortLabel")}
            </label>
            <input
              id="ws-port"
              type="number"
              min={1}
              max={65535}
              value={webSocketPort}
              onChange={handleWebSocketPortChange}
              className="w-full bg-transparent outline-none text-left"
            />
          </M3eFormField>
        </div>
      </div>
    </div>
  );
}
