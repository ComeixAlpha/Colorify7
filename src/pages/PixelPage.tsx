import {
  M3eButton,
  M3eFab,
  M3eFabMenu,
  M3eFabMenuItem,
  M3eFabMenuTrigger,
  M3eFormField,
  M3eIcon,
  M3eOption,
  M3eSelect,
  M3eSwitch,
  M3eTab,
  M3eTabs,
} from "@m3e/react/all";
import "@m3e/web/fab-menu";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ChangeEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import carpet_palette from "../assets/carpet_palette.json";
import pixel_palette from "../assets/pixel_palette.json";
import staircase_palette from "../assets/staircase_palette.json";
import TaskOverlay from "../components/TaskOverlay";
import WorldPicker, { type WorldInfo } from "../components/WorldPicker";
import { useIsNarrow } from "../hooks/useIsNarrow";
import { PageState } from "../session/PageState";
import { Storer } from "../stores/storer";
import PixelPalette, { type PixelPaletteProps } from "./PixelPalette";
import { AppSettings } from "./SettingsPage";

interface PixelPaletteJson {
  palette: { id: string; cn: string; average: [number, number, number] }[];
}

interface StaircasePaletteJson {
  data: Record<string, string>;
}

/** "#RRGGBB" -> [r, g, b]（0-255） */
function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", ""), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

type ResizeInterpolation =
  | "Nearest"
  | "Box"
  | "Bilinear"
  | "Hamming"
  | "CatmullRom"
  | "Mitchell"
  | "Gaussian"
  | "Lanczos3";

const INTERPOLATIONS: readonly ResizeInterpolation[] = [
  "Nearest",
  "Box",
  "Bilinear",
  "Hamming",
  "CatmullRom",
  "Mitchell",
  "Gaussian",
  "Lanczos3",
];

type GenerationPlane = "xOy" | "xOz" | "yOz";

const PLANES: readonly GenerationPlane[] = ["xOy", "xOz", "yOz"];

type ColorDistanceFormula =
  | "RGB"
  | "RGB+"
  | "Redmean"
  | "CMC l:c"
  | "CIE76"
  | "CIE94"
  | "CIEDE2000";

const FORMULAS: readonly ColorDistanceFormula[] = [
  "RGB",
  "RGB+",
  "Redmean",
  "CMC l:c",
  "CIE76",
  "CIE94",
  "CIEDE2000",
];

enum DitheringAlgorithm {
  Atkinson = "Atkinson",
  Burkes = "Burkes",
  FloydSteinberg = "FloydSteinberg",
  Stucki = "Stucki",
  JarvisJudiceNinke = "JarvisJudiceNinke",
  Sierra3 = "Sierra3",
}

interface ProgressMessage {
  stage: string;
  finished: boolean;
  elapsedMs?: number;
  outputDir?: string;
  error?: string | null;
}

interface ResultInfo {
  elapsedMs?: number;
  outputDir?: string;
}

interface WsStatus {
  running: boolean;
  port: number;
  connections: number;
}

export interface PixelParams {
  resizeX: number | null;
  resizeY: number | null;
  resizeInterpolation: ResizeInterpolation;
  generationPlane: GenerationPlane;
  colorDistanceFormula: ColorDistanceFormula;
  useStaircase: boolean;
  useStruct: boolean;
  useDithering: boolean;
  ditheringAlgorithm: DitheringAlgorithm;
  carpetOnly: boolean;
  woolOnly: boolean;
  noGlass: boolean;
  noSandsAndPowders: boolean;
  offsetX: number | null;
  offsetY: number | null;
  offsetZ: number | null;
  staircaseGap: number;
  staircaseCompress: boolean;
  wsCommandDelay: number;
  pkName: string;
  pkAuth: string;
  pkDesc: string;
  useLdb: boolean;
  worldPath: string;
  originX: number | null;
  originY: number | null;
  originZ: number | null;
}

export class PixelPageState extends PageState<PixelParams> {
  constructor() {
    super({
      resizeX: null,
      resizeY: null,
      resizeInterpolation: "Nearest",
      generationPlane: "xOy",
      colorDistanceFormula: "RGB",
      useStaircase: false,
      useStruct: false,
      useDithering: false,
      ditheringAlgorithm: DitheringAlgorithm.FloydSteinberg,
      carpetOnly: false,
      woolOnly: false,
      noGlass: false,
      noSandsAndPowders: false,
      offsetX: null,
      offsetY: null,
      offsetZ: null,
      staircaseGap: 2,
      staircaseCompress: true,
      wsCommandDelay: 10,
      pkName: "",
      pkAuth: "",
      pkDesc: "",
      useLdb: false,
      worldPath: "",
      originX: null,
      originY: null,
      originZ: null,
    });
  }
}

export const pixelPageState = new PixelPageState();

export default function PixelPage({
  onOpenWsPage,
}: {
  onOpenWsPage?: () => void;
}) {
  const { t } = useTranslation();
  const { data, update } = pixelPageState.use();
  const isAndroid = /Android/i.test(navigator.userAgent);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const cancelledRef = useRef(false);
  const modeRef = useRef<"file" | "socket" | "ldb">("file");
  const [processing, setProcessing] = useState(false);
  const [done, setDone] = useState(false);
  const [doneError, setDoneError] = useState(false);
  const [progressText, setProgressText] = useState("");
  const [resultInfo, setResultInfo] = useState<ResultInfo>({});
  const [wsMode, setWsMode] = useState(false);
  const [wsRunning, setWsRunning] = useState(false);
  const [worldChoices, setWorldChoices] = useState<WorldInfo[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerLoading, setPickerLoading] = useState(false);
  const [pickerError, setPickerError] = useState<string | null>(null);

  const isNarrow = useIsNarrow();
  const [activeTab, setActiveTab] = useState<"params" | "palette">("params");

  useEffect(() => {
    const id = setInterval(async () => {
      try {
        const s = await invoke<WsStatus>("ws_status");
        setWsRunning(s.running);
      } catch {}
    }, 1000);
    return () => clearInterval(id);
  }, []);

  function handleFilesClick() {
    modeRef.current = "file";
    fileInputRef.current?.click();
  }

  function handleWsClick() {
    modeRef.current = "socket";
    fileInputRef.current?.click();
  }

  function handleLdbClick() {
    modeRef.current = "ldb";
    fileInputRef.current?.click();
  }

  /// 浏览世界文件夹（直写 LevelDB 用）：桌面用系统目录选择器，安卓用世界发现列表
  async function handleBrowseWorld() {
    if (isAndroid) {
      setPickerLoading(true);
      setPickerOpen(true);
      setPickerError(null);
      try {
        const worlds = await invoke<WorldInfo[]>("ldb_list_world_dirs");
        setWorldChoices(worlds);
      } catch (err) {
        console.error("ldb_discover_worlds failed:", err);
        setWorldChoices([]);
        setPickerError(typeof err === "string" ? err : String(err));
      } finally {
        setPickerLoading(false);
      }
      return;
    }
    try {
      const path = await open({
        title: t("pages.pixel.selectWorldTitle"),
        directory: true,
      });
      if (typeof path === "string" && path) {
        update({ worldPath: path });
      }
    } catch (err) {
      console.error("open world dialog failed:", err);
      setPickerError(typeof err === "string" ? err : String(err));
    }
  }

  async function handleFileChange(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0] ?? null;
    if (!file) {
      e.target.value = "";
      return;
    }
    const bytes = new Uint8Array(await file.arrayBuffer());

    // 订阅后端进度消息
    const channel = new Channel<ProgressMessage>();
    console.log("Channel id:", channel.id);
    cancelledRef.current = false;
    channel.onmessage = (msg) => {
      console.log("Received progress message:", msg);
      if (msg.finished) {
        if (msg.error) {
          setDone(true);
          setDoneError(true);
          setProgressText(msg.error);
        } else if (!cancelledRef.current) {
          setDone(true);
          setDoneError(false);
          setResultInfo({ elapsedMs: msg.elapsedMs, outputDir: msg.outputDir });
          setProgressText(t("pages.pixel.taskComplete"));
        }
      } else {
        setProgressText(msg.stage);
      }
    };

    const paletteProps = await Storer.loadPref<PixelPaletteProps>(
      "pixel_palette_props",
    );
    const palette = data.useStaircase
      ? Object.entries((staircase_palette as StaircasePaletteJson).data).map(
          ([key, hex]) => ({
            id: key.replace("minecraft:", ""),
            average: hexToRgb(hex),
          }),
        )
      : (data.carpetOnly
          ? (carpet_palette as PixelPaletteJson).palette
          : (pixel_palette as PixelPaletteJson).palette
        )
          .filter((p) => {
            if (paletteProps.disabledIds.includes(p.id)) {
              return false;
            }

            if (data.woolOnly && !p.id.includes("wool")) {
              return false;
            }

            if (data.noGlass && p.id.includes("glass")) {
              return false;
            }

            if (
              data.noSandsAndPowders &&
              (p.id.includes("sand") || p.id.includes("powder"))
            ) {
              return false;
            }

            return true;
          })
          .map((p) => ({ id: p.id, average: p.average }));

    setProcessing(true);
    setDone(false);
    setDoneError(false);
    setWsMode(modeRef.current === "socket");
    setProgressText("正在准备...");
    try {
      await invoke("process_image", {
        image: bytes,
        params: {
          ...data,
          useSocket: modeRef.current === "socket",
          useLdb: modeRef.current === "ldb",
          worldPath: modeRef.current === "ldb" ? data.worldPath : null,
          originX: modeRef.current === "ldb" ? data.originX : null,
          originY: modeRef.current === "ldb" ? data.originY : null,
          originZ: modeRef.current === "ldb" ? data.originZ : null,
          autoSliceMcfunction:
            Storer.load<AppSettings>("settings").autoSliceMcfunction,
          previewImage: Storer.load<AppSettings>("settings").previewImage,
          websocketPort: Storer.load<AppSettings>("settings").webSocketPort,
        },
        palette,
        onProgress: channel,
      });
      console.log("process_image resolved, 后台线程继续运行");
    } catch (err) {
      console.error("process_image failed:", err);
      setDone(true);
      setDoneError(true);
      setProgressText(typeof err === "string" ? err : String(err));
    }
    console.log("Sent image to backend:", file.name);
    e.target.value = "";
  }

  async function handleCancel() {
    cancelledRef.current = true;
    setProcessing(false);
    setDone(false);
    try {
      await invoke("cancel_process");
    } catch (err) {
      console.error("cancel_process failed:", err);
    }
  }

  function handleDoneOk() {
    setProcessing(false);
    setDone(false);
  }

  function handleWsGoPage() {
    handleDoneOk();
    onOpenWsPage?.();
  }

  function handleNumberChange(field: keyof PixelParams) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      if (raw === "") {
        update({ [field]: null } as Partial<PixelParams>);
        return;
      }
      const n = Number(raw);
      if (Number.isNaN(n)) {
        update({ [field]: null } as Partial<PixelParams>);
        return;
      } else {
      }
      update({ [field]: n } as Partial<PixelParams>);
    };
  }

  function handleTextChange(field: keyof PixelParams) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      update({ [field]: raw } as Partial<PixelParams>);
    };
  }

  function handleEnumChange<T>(
    field: keyof PixelParams,
    enums: readonly T[],
  ): (e: Event) => void {
    return (e: Event) => {
      const value = (e.target as any)?.value as T;
      if (!enums.includes(value)) return;
      update({ [field]: value } as Partial<PixelParams>);
    };
  }

  function handleSwitchChange(field: keyof PixelParams): (e: Event) => void {
    return (e: Event) => {
      const checked = (e.target as any)?.checked === true;
      update({ [field]: checked } as Partial<PixelParams>);
    };
  }

  /// 仅地毯 / 仅羊毛互斥
  function handleCarpetOnlyChange(e: Event) {
    const checked = (e.target as any)?.checked === true;
    update({ carpetOnly: checked, woolOnly: checked ? false : data.woolOnly });
  }

  function handleWoolOnlyChange(e: Event) {
    const checked = (e.target as any)?.checked === true;
    update({
      woolOnly: checked,
      carpetOnly: checked ? false : data.carpetOnly,
    });
  }

  return (
    <div className="w-full h-full relative flex flex-col">
      {/* 窄屏 */}
      {isNarrow && (
        <M3eTabs stretch className="m3e-tabs-compact mb-2 shrink-0">
          <M3eTab
            htmlFor="pixel-params"
            selected={activeTab === "params"}
            onClick={() => setActiveTab("params")}
          >
            {t("pages.pixel.tabParams")}
          </M3eTab>
          <M3eTab
            htmlFor="pixel-palette"
            selected={activeTab === "palette"}
            onClick={() => setActiveTab("palette")}
          >
            {t("pages.pixel.tabPalette")}
          </M3eTab>
        </M3eTabs>
      )}

      {/* 宽屏 */}
      <div
        className={
          isNarrow ? "flex flex-1 min-h-0 p-6" : "flex flex-1 gap-4 min-h-0 p-6"
        }
      >
        {/* 参数表 */}
        <div
          className={
            isNarrow && activeTab !== "params"
              ? "hidden"
              : "flex-1 flex flex-col gap-6 items-start overflow-y-auto scrollbar-thin"
          }
          style={{ justifyContent: "safe center" }}
        >
          {/* 裁剪 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argResize")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argResizeDesc")}
                </span>
              </div>

              <div className="flex gap-4 pt-2">
                <M3eFormField className="w-40 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="resize-x">
                    X
                  </label>
                  <input
                    id="resize-x"
                    type="number"
                    min={1}
                    value={data.resizeX ?? ""}
                    onChange={handleNumberChange("resizeX")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField className="w-40 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="resize-y">
                    Y
                  </label>
                  <input
                    id="resize-y"
                    type="number"
                    min={1}
                    value={data.resizeY ?? ""}
                    onChange={handleNumberChange("resizeY")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>

          {/* 裁剪插值法 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argResizeInterpolation")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argResizeInterpolationDesc")}
                </span>
              </div>

              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="resize-interpolation-select"
                  onChange={handleEnumChange(
                    "resizeInterpolation",
                    INTERPOLATIONS,
                  )}
                >
                  {INTERPOLATIONS.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.resizeInterpolation === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 平面 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argGenerationPlane")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argGenerationPlaneDesc")}
                </span>
              </div>

              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="generation-plane-select"
                  onChange={handleEnumChange("generationPlane", PLANES)}
                >
                  {PLANES.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.generationPlane === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 色差公式 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argColorDistance")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argColorDistanceDesc")}
                </span>
              </div>

              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="color-distance-formula-select"
                  onChange={handleEnumChange("colorDistanceFormula", FORMULAS)}
                >
                  {FORMULAS.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.colorDistanceFormula === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 阶梯式 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argStaircase")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argStaircaseDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.useStaircase}
                onChange={handleSwitchChange("useStaircase")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 阶梯式无损压缩 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argStaircaseCompress")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argStaircaseCompressDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.staircaseCompress}
                onChange={handleSwitchChange("staircaseCompress")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 结构 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argUseStruct")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argUseStructDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.useStruct}
                onChange={handleSwitchChange("useStruct")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 抖动 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argDithering")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argDitheringDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.useDithering}
                onChange={handleSwitchChange("useDithering")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 抖动算法 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argDitheringAlgorithm")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argDitheringAlgorithmDesc")}
                </span>
              </div>

              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="color-distance-formula-select"
                  onChange={handleEnumChange(
                    "ditheringAlgorithm",
                    Object.values(DitheringAlgorithm),
                  )}
                >
                  {Object.values(DitheringAlgorithm).map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.ditheringAlgorithm === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 仅地毯 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argCarpetOnly")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argCarpetOnlyDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.carpetOnly}
                onChange={handleCarpetOnlyChange}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 仅羊毛 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argWoolOnly")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argWoolOnlyDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.woolOnly}
                onChange={handleWoolOnlyChange}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 无玻璃 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argNoGlass")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argNoGlassDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.noGlass}
                onChange={handleSwitchChange("noGlass")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 无沙子与粉末 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argNoSandsAndPowders")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argNoSandsAndPowdersDesc")}
                </span>
              </div>

              <M3eSwitch
                checked={data.noSandsAndPowders}
                onChange={handleSwitchChange("noSandsAndPowders")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 偏移 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argOffset")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argOffsetDesc")}
                </span>
              </div>

              <div className="flex gap-4 pt-2">
                <M3eFormField className="w-20 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="offset-x">
                    X
                  </label>
                  <input
                    id="offset-x"
                    type="number"
                    value={data.offsetX ?? ""}
                    onChange={handleNumberChange("offsetX")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField className="w-20 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="offset-y">
                    Y
                  </label>
                  <input
                    id="offset-y"
                    type="number"
                    value={data.offsetY ?? ""}
                    onChange={handleNumberChange("offsetY")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField className="w-20 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="offset-z">
                    Z
                  </label>
                  <input
                    id="offset-z"
                    type="number"
                    value={data.offsetZ ?? ""}
                    onChange={handleNumberChange("offsetZ")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>

          {/* 阶梯式竖向间隔 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argStaircaseGap")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argStaircaseGapDesc")}
                </span>
              </div>

              <div className="flex gap-4 pt-2">
                <M3eFormField className="w-40 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="staircase-gap">
                    Gap
                  </label>
                  <input
                    id="staircase-gap"
                    type="number"
                    min={1}
                    value={data.staircaseGap ?? ""}
                    onChange={handleNumberChange("staircaseGap")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>

          {/* WS命令间隔 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.pixel.argWebSocketCommandDelay")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argWebSocketCommandDelayDesc")}
                </span>
              </div>

              <div className="flex gap-4 pt-2">
                <M3eFormField className="w-40 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="websocket-command-delay">
                    Delay
                  </label>
                  <input
                    id="websocket-command-delay"
                    type="number"
                    min={1}
                    value={data.wsCommandDelay ?? ""}
                    onChange={handleNumberChange("wsCommandDelay")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>

          {/* 直写 LevelDB 世界 */}
          <div className="w-full flex items-center justify-between gap-4">
            <div className="w-full flex items-center justify-between gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">{t("pages.pixel.argLdb")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argLdbDesc")}
                </span>
              </div>
              <M3eSwitch
                checked={data.useLdb}
                onChange={handleSwitchChange("useLdb")}
                className="shrink-0"
              />
            </div>
          </div>
          {data.useLdb && (
            <div className="flex w-full min-w-0 flex-col gap-3">
              {/* 世界路径 */}
              <div className="flex w-full min-w-0 items-center justify-between gap-4">
                <div className="flex flex-col min-w-0 flex-1">
                  <span className="font-medium">
                    {t("pages.pixel.argLdbWorldPath")}
                  </span>
                  <span className="text-sm text-md-on-surface-variant truncate">
                    {data.worldPath ||
                      t("pages.pixel.argLdbWorldPathPlaceholder")}
                  </span>
                </div>
                <M3eButton
                  variant="tonal"
                  size="small"
                  className="shrink-0"
                  onClick={handleBrowseWorld}
                >
                  {t("pages.pixel.argLdbBrowse")}
                </M3eButton>
              </div>
              {/* 生成坐标 */}
              <div className="flex flex-col gap-2">
                <span className="font-medium">
                  {t("pages.pixel.argLdbOrigin")}
                </span>
                <div className="flex w-full min-w-0 gap-2">
                  <M3eFormField
                    className="flex-1 shrink-0"
                    hideSubscript="always"
                  >
                    <label slot="label" htmlFor="ldb-origin-x">
                      X
                    </label>
                    <input
                      id="ldb-origin-x"
                      type="number"
                      value={data.originX ?? ""}
                      onChange={handleNumberChange("originX")}
                      className="w-full bg-transparent outline-none text-left"
                    />
                  </M3eFormField>
                  <M3eFormField
                    className="flex-1 shrink-0"
                    hideSubscript="always"
                  >
                    <label slot="label" htmlFor="ldb-origin-y">
                      Y
                    </label>
                    <input
                      id="ldb-origin-y"
                      type="number"
                      value={data.originY ?? ""}
                      onChange={handleNumberChange("originY")}
                      className="w-full bg-transparent outline-none text-left"
                    />
                  </M3eFormField>
                  <M3eFormField
                    className="flex-1 shrink-0"
                    hideSubscript="always"
                  >
                    <label slot="label" htmlFor="ldb-origin-z">
                      Z
                    </label>
                    <input
                      id="ldb-origin-z"
                      type="number"
                      value={data.originZ ?? ""}
                      onChange={handleNumberChange("originZ")}
                      className="w-full bg-transparent outline-none text-left"
                    />
                  </M3eFormField>
                </div>
              </div>
            </div>
          )}

          {/* 打包 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0 w-full">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">{t("pages.pixel.argPack")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.pixel.argPackDesc")}
                </span>
              </div>

              <div className="flex flex-col gap-3 pt-2 w-full">
                <M3eFormField
                  className="w-full shrink-0"
                  hideSubscript="always"
                >
                  <label slot="label" htmlFor="pk-name">
                    {t("pages.pixel.argPackName")}
                  </label>
                  <input
                    id="pk-name"
                    type="text"
                    value={data.pkName}
                    onChange={handleTextChange("pkName")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField
                  className="w-full shrink-0"
                  hideSubscript="always"
                >
                  <label slot="label" htmlFor="pk-auth">
                    {t("pages.pixel.argPackAuth")}
                  </label>
                  <input
                    id="pk-auth"
                    type="text"
                    value={data.pkAuth}
                    onChange={handleTextChange("pkAuth")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField
                  className="w-full shrink-0"
                  hideSubscript="always"
                >
                  <label slot="label" htmlFor="pk-desc">
                    {t("pages.pixel.argPackDescField")}
                  </label>
                  <input
                    id="pk-desc"
                    type="text"
                    value={data.pkDesc}
                    onChange={handleTextChange("pkDesc")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>
        </div>

        {/* 调色板 */}
        <div
          className={
            isNarrow && activeTab !== "palette"
              ? "hidden"
              : "flex flex-1 relative w-full min-h-0"
          }
        >
          <div className="w-full h-full">
            <PixelPalette carpetOnly={data.carpetOnly} />
          </div>
        </div>
      </div>

      {/* FAB */}
      <div className="absolute right-2 bottom-2">
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={handleFileChange}
        />
        <M3eFab variant="primary" size="medium">
          <M3eFabMenuTrigger htmlFor="fabmenu">
            <M3eIcon name="play_arrow" />
          </M3eFabMenuTrigger>
        </M3eFab>
        <M3eFabMenu id="fabmenu">
          <M3eFabMenuItem onClick={handleFilesClick}>
            <M3eIcon slot="icon" name="file_copy" filled />
            {t("pages.pixel.fabGenerateFiles")}
          </M3eFabMenuItem>
          <div title={wsRunning ? undefined : t("pages.pixel.wsNeedsServer")}>
            <M3eFabMenuItem disabled={!wsRunning} onClick={handleWsClick}>
              <M3eIcon slot="icon" name="cable" filled />
              {t("pages.pixel.fabGenerateWs")}
            </M3eFabMenuItem>
          </div>
          <div
            title={
              data.useLdb && data.worldPath
                ? undefined
                : t("pages.pixel.ldbNeedsWorld")
            }
          >
            <M3eFabMenuItem
              disabled={!data.useLdb || !data.worldPath}
              onClick={handleLdbClick}
            >
              <M3eIcon slot="icon" name="storage" filled />
              {t("pages.pixel.fabGenerateLdb")}
            </M3eFabMenuItem>
          </div>
        </M3eFabMenu>
      </div>

      <TaskOverlay
        processing={processing}
        done={done}
        error={doneError}
        progressText={progressText}
        wsMode={wsMode}
        resultInfo={resultInfo}
        wsHint={t("pages.pixel.wsGoPageHint")}
        wsGoPageLabel={t("pages.pixel.wsGoPage")}
        wsNoThanksLabel={t("pages.pixel.wsNoThanks")}
        elapsedLabel={t("pages.pixel.elapsedLabel")}
        outputLabel={t("pages.pixel.outputLabel")}
        cancelLabel="取消"
        onCancel={handleCancel}
        onDoneOk={handleDoneOk}
        onWsGoPage={handleWsGoPage}
      />

      <WorldPicker
        open={pickerOpen}
        worlds={worldChoices}
        loading={pickerLoading}
        error={pickerError ?? undefined}
        title={t("pages.pixel.selectWorldTitle")}
        emptyText={t("pages.pixel.argLdbNoWorld")}
        hint={t("pages.pixel.argLdbWorldDirHint")}
        onOpenSettings={
          isAndroid
            ? () => {
                invoke("open_all_files_settings").catch((err) =>
                  console.error("open settings failed:", err),
                );
              }
            : undefined
        }
        settingsLabel={t("pages.pixel.argLdbOpenSettings")}
        onSelect={(path) => {
          update({ worldPath: path });
          setPickerOpen(false);
        }}
        onClose={() => setPickerOpen(false)}
      />
    </div>
  );
}
