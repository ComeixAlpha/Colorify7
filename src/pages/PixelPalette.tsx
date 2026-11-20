import { M3eFilterChip, M3eFormField } from "@m3e/react/all";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import carpet_palette from "../assets/carpet_palette.json";
import pixel_palette from "../assets/pixel_palette_v2.json";
import PixelPaletteTile from "../components/PixelPaletteTile";
import { isAvailable } from "../lib/versions";
import {
  getDisabledIds,
  loadDisabledIds,
  PIXEL_PALETTE_PROPS_CHANGED_EVENT,
  setBlockDisabled,
} from "../stores/pixelPaletteProps";

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

type PaletteItem = IPixelPalette["palette"][number];

const pixelPalette: IPixelPalette = pixel_palette as IPixelPalette;
const carpetPalette: IPixelPalette = carpet_palette as IPixelPalette;

type PaletteFilter = "enabled" | "disabled";

const LEAVE_DURATION_MS = 220;

const ITEM_GAP_CLASS = "pb-6";

/**
 * 正在从列表里消失的条目。
 */
function LeavingPaletteTile({
  upColor,
  sideColor,
  id,
  name,
  dimmed,
  onDone,
}: {
  upColor: string;
  sideColor: string;
  id: string;
  name: string;
  dimmed: boolean;
  onDone: () => void;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setHeight(el.getBoundingClientRect().height);
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(() => setCollapsed(true));
    });
    return () => {
      cancelAnimationFrame(raf1);
      cancelAnimationFrame(raf2);
    };
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(
      () => onDoneRef.current(),
      LEAVE_DURATION_MS + 80,
    );
    return () => window.clearTimeout(timer);
  }, []);

  return (
    <div
      ref={ref}
      aria-hidden="true"
      className="pointer-events-none w-full shrink-0 overflow-hidden"
      style={{
        height: collapsed ? 0 : (height ?? undefined),
        opacity: collapsed ? 0 : 1,
        transition: `height ${LEAVE_DURATION_MS}ms ease-out, opacity ${LEAVE_DURATION_MS}ms ease-out`,
      }}
    >
      <div className={ITEM_GAP_CLASS}>
        <PixelPaletteTile
          upColor={upColor}
          sideColor={sideColor}
          id={id}
          name={name}
          disabled={dimmed}
          onToggle={() => {}}
        />
      </div>
    </div>
  );
}

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
  const [filter, setFilter] = useState<PaletteFilter>("enabled");
  const [disabledIds, setDisabledIds] = useState<string[]>(getDisabledIds);
  const [leavingIds, setLeavingIds] = useState<string[]>([]);

  useEffect(() => {
    let alive = true;
    void loadDisabledIds().then((ids) => {
      if (alive) setDisabledIds(ids);
    });

    const onChange = (e: Event) => {
      const detail = (e as CustomEvent<{ disabledIds?: string[] }>).detail;
      setDisabledIds(detail?.disabledIds ?? getDisabledIds());
    };
    window.addEventListener(PIXEL_PALETTE_PROPS_CHANGED_EVENT, onChange);
    return () => {
      alive = false;
      window.removeEventListener(PIXEL_PALETTE_PROPS_CHANGED_EVENT, onChange);
    };
  }, []);

  useLayoutEffect(() => {
    setLeavingIds((prev) => (prev.length === 0 ? prev : []));
  }, [filter, carpetOnly, targetVersion]);

  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSearch(e.target.value);
  };

  const available = basePalette.palette.filter((item) =>
    isAvailable(item.since, targetVersion),
  );

  const query = search.toLowerCase();
  const matchesSearch = (item: PaletteItem) =>
    query === "" ||
    item.id.toLowerCase().includes(query) ||
    item.cn.toLowerCase().includes(query);
  const matchesFilter = (item: PaletteItem) =>
    filter === "enabled"
      ? !disabledIds.includes(item.id)
      : disabledIds.includes(item.id);

  const visibleIds = new Set(
    available
      .filter((item) => matchesFilter(item) && matchesSearch(item))
      .map((item) => item.id),
  );
  const leavingSet = new Set(leavingIds);
  const rendered = available.filter(
    (item) => visibleIds.has(item.id) || leavingSet.has(item.id),
  );

  const totalCount = available.length;
  const disabledCount = available.filter((p) =>
    disabledIds.includes(p.id),
  ).length;

  function handleToggle(id: string, nextDisabled: boolean) {
    setDisabledIds((prev) =>
      nextDisabled
        ? prev.includes(id)
          ? prev
          : [...prev, id]
        : prev.filter((item) => item !== id),
    );
    setLeavingIds((prev) => (prev.includes(id) ? prev : [...prev, id]));

    setBlockDisabled(id, nextDisabled)
      .then((ids) => setDisabledIds(ids))
      .catch((err) => {
        // 写入失败
        console.error("persist palette disabled state failed:", err);
        setLeavingIds((prev) => prev.filter((item) => item !== id));
        void loadDisabledIds().then((ids) => setDisabledIds(ids));
      });
  }

  function handleLeaveDone(id: string) {
    setLeavingIds((prev) => prev.filter((item) => item !== id));
  }

  const handleFilterBeforeInput = (next: PaletteFilter) => (e: Event) => {
    e.preventDefault();
    setFilter(next);
  };

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
            {/* 筛选栏 */}
            <div
              className="mt-3 flex flex-wrap gap-2"
              role="group"
              aria-label={t("pages.pixel_palette.filterLabel")}
            >
              <M3eFilterChip
                selected={filter === "enabled"}
                onBeforeInput={handleFilterBeforeInput("enabled")}
              >
                {t("pages.pixel_palette.filterEnabled")}
              </M3eFilterChip>
              <M3eFilterChip
                selected={filter === "disabled"}
                onBeforeInput={handleFilterBeforeInput("disabled")}
              >
                {t("pages.pixel_palette.filterDisabled")}
              </M3eFilterChip>
            </div>
            <span className="mt-1 block text-sm text-md-on-surface-variant">
              {t("pages.pixel_palette.count", {
                total: totalCount,
                disabled: disabledCount,
              })}
            </span>
          </div>
        </div>
      </div>
      <div className="flex flex-1 flex-col min-h-0 items-start justify-start overflow-y-auto scrollbar-thin">
        {rendered.length === 0 && (
          <p className="text-sm text-md-on-surface-variant">
            {t("pages.pixel_palette.filterEmpty")}
          </p>
        )}
        {rendered.map((item) => {
          const up =
            (colorSpace === "srgb" ? (item.upSrgb ?? item.up) : item.up) ??
            item.average;
          const side =
            (colorSpace === "srgb"
              ? (item.sideSrgb ?? item.side)
              : item.side) ?? item.average;
          const colors = {
            upColor: `rgb(${up.join(", ")})`,
            sideColor: `rgb(${side.join(", ")})`,
            id: item.id,
            name: item.cn,
          };

          if (leavingSet.has(item.id)) {
            return (
              <LeavingPaletteTile
                key={item.id}
                {...colors}
                dimmed={filter === "disabled"}
                onDone={() => handleLeaveDone(item.id)}
              />
            );
          }

          return (
            <div key={item.id} className={`w-full shrink-0 ${ITEM_GAP_CLASS}`}>
              <PixelPaletteTile
                {...colors}
                disabled={disabledIds.includes(item.id)}
                onToggle={(next) => handleToggle(item.id, next)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
