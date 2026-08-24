import i18n from "i18next";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Storer } from "../stores/storer";
import { PixelPaletteProps } from "../pages/PixelPalette";



export default function PixelPaletteTile({
  color,
  id,
  name,
  disabled
}: {
  color: string;
  id: string;
  name: string;
  disabled: boolean;
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

  const [isDisabled, setIsDisabled] = useState(disabled);

  function handleClick() {
    setIsDisabled(!isDisabled);
    Storer.savePartial("pixel_palette_props", {
      disabledIds: isDisabled
        ? Storer.load<PixelPaletteProps>("pixel_palette_props").disabledIds.filter(
            (item: string) => item !== id,
          )
        : [...Storer.load<PixelPaletteProps>("pixel_palette_props").disabledIds, id],
    });
  }

  return (
    <div className="relative w-full" onClick={handleClick}>
      <div className="flex items-center gap-2 ">
        <div
          className="w-12 h-12 rounded-lg"
          style={{ backgroundColor: color }}
        />
        {display}
      </div>
      <div
        className={`
          absolute inset-0 bg-black/50 grid place-items-center 
          transition-opacity duration-120
          ${isDisabled ? "opacity-100 pointer-events-auto" : "opacity-0 pointer-events-none"}
        `}
      >
        <p className="--color-md-error text-center">{t("pages.pixel_palette.disabled")}</p>
      </div>
    </div>
  );
}
