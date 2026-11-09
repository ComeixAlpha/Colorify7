import { M3eFormField } from "@m3e/react/all";
import { useEffect, useReducer, useState } from "react";
import { useTranslation } from "react-i18next";
import carpet_palette from "../assets/carpet_palette.json";
import pixel_palette from "../assets/pixel_palette_v2.json";
import PixelPaletteTile from "../components/PixelPaletteTile";
import { isAvailable } from "../lib/versions";
import { Storer } from "../stores/storer";

interface IPixelPalette {
  palette: {
    id: string;
    cn: string;
    average: number[];
    up?: number[];
    side?: number[];
    upSrgb?: number[];
    sideSrgb?: number[];
    since?: string | null;
  }[];
}

export interface PixelPaletteProps {
  disabledIds: string[];
}

const DEFAULT_PIXEL_PALETTE_PROPS: PixelPaletteProps = {
  disabledIds: ["tnt"],
};

Storer.registerDefaults("pixel_palette_props", DEFAULT_PIXEL_PALETTE_PROPS);

const pixelPalette: IPixelPalette = pixel_palette as IPixelPalette;
const carpetPalette: IPixelPalette = carpet_palette as IPixelPalette;

export default function PixelPalette({
  carpetOnly = false,
  colorSpace = "linear",
  targetVersion = "latest",
}: {
  carpetOnly?: boolean;
  colorSpace?: "linear" | "srgb";
  targetVersion?: string;
}) {
  const { t } = useTranslation();

  const basePalette = carpetOnly ? carpetPalette : pixelPalette;
  const [search, setSearch] = useState("");
  // 子项切换禁用后需要让计数跟着更新
  const [, forceRender] = useReducer((x: number) => x + 1, 0);
  useEffect(() => {
    const onChange = () => forceRender();
    window.addEventListener("colorify:palette-props-changed", onChange);
    return () =>
      window.removeEventListener("colorify:palette-props-changed", onChange);
  }, []);

  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSearch(e.target.value);
  };

  const available = basePalette.palette.filter((item) =>
    isAvailable(item.since, targetVersion),
  );

  const palette: IPixelPalette =
    search === ""
      ? { palette: available }
      : {
          palette: available.filter((item) => {
            const q = search.toLowerCase();
            const idMatch = item.id.toLowerCase().includes(q);
            const nameMatch = item.cn.toLowerCase().includes(q);
            return idMatch || nameMatch;
          }),
        };

  const props = Storer.load<PixelPaletteProps>("pixel_palette_props");
  const totalCount = available.length;
  const disabledCount = available.filter((p) =>
    props.disabledIds.includes(p.id),
  ).length;

  return (
    <div className="w-full h-full flex flex-col gap-4">
      {/* 阶梯式竖向间隔 */}
      <div className="flex w-full items-center justify-between gap-4">
        <div className="w-full gap-3 min-w-0">
          <div className="flex flex-col min-w-0">
            <span className="font-medium">
              {t("pages.pixel_palette.search")}
            </span>
            <span className="text-sm text-md-on-surface-variant">
              {t("pages.pixel_palette.searchDesc")}
            </span>
          </div>

          <div className="pt-2">
            <M3eFormField className="w-full shrink-0" hideSubscript="always">
              <label slot="label" htmlFor="search">
                ID/Name
              </label>
              <input
                id="search"
                type="text"
                value={search}
                onChange={handleSearchChange}
                className="w-full bg-transparent outline-none text-left"
              />
            </M3eFormField>
            <span className="mt-1 block text-sm text-md-on-surface-variant">
              {t("pages.pixel_palette.count", {
                total: totalCount,
                disabled: disabledCount,
              })}
            </span>
          </div>
        </div>
      </div>
      <div
        className="flex flex-1 flex-col min-h-0 gap-6 items-start overflow-y-auto scrollbar-thin"
        style={{ justifyContent: "safe center" }}
      >
        {palette.palette.map((item) => {
          const up =
            (colorSpace === "srgb" ? item.upSrgb ?? item.up : item.up) ??
            item.average;
          const side =
            (colorSpace === "srgb"
              ? item.sideSrgb ?? item.side
              : item.side) ?? item.average;
          return (
            <PixelPaletteTile
              key={item.id}
              upColor={`rgb(${up.join(", ")})`}
              sideColor={`rgb(${side.join(", ")})`}
              id={item.id}
              name={item.cn}
              disabled={props.disabledIds.includes(item.id)}
            />
          );
        })}
      </div>
    </div>
  );
}
