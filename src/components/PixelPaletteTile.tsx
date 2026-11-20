import i18n from "i18next";
import { useTranslation } from "react-i18next";

export default function PixelPaletteTile({
  upColor,
  sideColor,
  id,
  name,
  disabled,
  onToggle,
}: {
  upColor: string;
  sideColor: string;
  id: string;
  name: string;
  disabled: boolean;
  onToggle: (nextDisabled: boolean) => void;
}) {
  const { t } = useTranslation();

  const lang = i18n.language == "en" ? "en" : "zh";
  const display =
    lang == "en" ? (
      <div>
        <p className="text-md-on-surface-variant">{id.replaceAll("_", " ")}</p>
        <p className="text-sm text-md-on-surface-variant">minecraft:{id}</p>
      </div>
    ) : (
      <div>
        <p className="text-md-on-surface-variant">{name}</p>
        <p className="text-sm text-md-on-surface-variant">minecraft:{id}</p>
      </div>
    );

  return (
    <div className="relative w-full" onClick={() => onToggle(!disabled)}>
      <div className="flex items-center gap-2">
        <div
          className={`flex min-w-0 items-center gap-2 ${
            disabled ? "opacity-60" : ""
          }`}
        >
          <div
            className="w-12 h-12 shrink-0 rounded-lg"
            title={t("pages.pixel_palette.swatchHint")}
            style={{
              background: `linear-gradient(135deg, ${upColor} 0 50%, ${sideColor} 50% 100%)`,
            }}
          />
          {display}
        </div>
        {disabled && (
          <span className="ml-2 shrink-0 rounded-full bg-md-error-container px-2 py-0.5 text-xs text-md-on-error-container">
            {t("pages.pixel_palette.disabled")}
          </span>
        )}
      </div>
    </div>
  );
}
