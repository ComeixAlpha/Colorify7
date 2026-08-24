import {
  M3eButton,
  M3eCircularProgressIndicator,
  M3eFab,
  M3eFabMenu,
  M3eFabMenuItem,
  M3eFabMenuTrigger,
  M3eFormField,
  M3eIcon,
  M3eOption,
  M3eSelect,
  M3eTab,
  M3eTabs,
} from "@m3e/react/all";
import "@m3e/web/fab-menu";
import { Channel, invoke } from "@tauri-apps/api/core";
import type { ChangeEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useIsNarrow } from "../hooks/useIsNarrow";
import { PageState } from "../session/PageState";
import ParticleMappings, { loadParticleMappings } from "./ParticleMappings";

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

type GenerationMode = "Match" | "Dust";

const MODES: readonly GenerationMode[] = ["Match", "Dust"];

export interface ParticleParams {
  resizeX: number | null;
  resizeY: number | null;
  height: number | null;
  resizeInterpolation: ResizeInterpolation;
  generationPlane: GenerationPlane;
  generationMode: GenerationMode;
  rx: number | null;
  ry: number | null;
  rz: number | null;
  pkName: string;
  pkAuth: string;
  pkDesc: string;
  wsCommandDelay: number | null;
}

export class ParticlePageState extends PageState<ParticleParams> {
  constructor() {
    super({
      resizeX: null,
      resizeY: null,
      height: null,
      resizeInterpolation: "Nearest",
      generationPlane: "xOy",
      generationMode: "Match",
      rx: null,
      ry: null,
      rz: null,
      pkName: "",
      pkAuth: "",
      pkDesc: "",
      wsCommandDelay: 10,
    });
  }
}

export const particlePageState = new ParticlePageState();

type NumberField =
  | "resizeX"
  | "resizeY"
  | "height"
  | "rx"
  | "ry"
  | "rz"
  | "wsCommandDelay";

interface ProgressMessage {
  stage: string;
  finished: boolean;
  elapsedMs?: number;
  outputDir?: string;
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

function formatElapsed(ms: number): string {
  const s = ms / 1000;
  if (s < 60) {
    return `${s.toFixed(1)} s`;
  }
  const m = Math.floor(s / 60);
  const rest = Math.round(s % 60);
  return `${m} min ${rest} s`;
}

export default function ParticlePage({
  onOpenWsPage,
}: {
  onOpenWsPage?: () => void;
}) {
  const { t } = useTranslation();
  const { data, update } = particlePageState.use();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const cancelledRef = useRef(false);
  const modeRef = useRef<"file" | "socket">("file");
  const [processing, setProcessing] = useState(false);
  const [done, setDone] = useState(false);
  const [progressText, setProgressText] = useState("");
  const [resultInfo, setResultInfo] = useState<ResultInfo>({});
  const [wsMode, setWsMode] = useState(false);
  const [wsRunning, setWsRunning] = useState(false);

  const isNarrow = useIsNarrow();
  const [activeTab, setActiveTab] = useState<"params" | "palette">("params");

  useEffect(() => {
    const id = setInterval(async () => {
      try {
        const s = await invoke<WsStatus>("ws_status");
        setWsRunning(s.running);
      } catch {
        /* 忽略 */
      }
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
        if (!cancelledRef.current) {
          setDone(true);
          setResultInfo({ elapsedMs: msg.elapsedMs, outputDir: msg.outputDir });
          setProgressText(t("pages.particle.taskComplete"));
        }
      } else {
        setProgressText(msg.stage);
      }
    };

    // 自定义映射
    const mappings = await loadParticleMappings();

    setProcessing(true);
    setDone(false);
    setWsMode(modeRef.current === "socket");
    setProgressText("正在准备...");
    try {
      await invoke("process_particle", {
        image: bytes,
        params: {
          ...data,
          useSocket: modeRef.current === "socket",
          mappings,
        },
        onProgress: channel,
      });
      console.log("process_particle resolved, 后台线程继续运行");
    } catch (err) {
      console.error("process_particle failed:", err);
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
      await invoke("cancel_particle_process");
    } catch (err) {
      console.error("cancel_particle_process failed:", err);
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

  function handleNumberChange(field: NumberField) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      if (raw === "") {
        update({ [field]: null } as Partial<ParticleParams>);
        return;
      }
      const n = Number(raw);
      if (Number.isNaN(n)) return;
      update({ [field]: n } as Partial<ParticleParams>);
    };
  }

  function handleTextChange(field: keyof ParticleParams) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      update({ [field]: raw } as Partial<ParticleParams>);
    };
  }

  function handleEnumChange<T>(
    field: keyof ParticleParams,
    enums: readonly T[],
  ): (e: Event) => void {
    return (e: Event) => {
      const value = (e.target as any)?.value as T;
      if (!enums.includes(value)) return;
      update({ [field]: value } as Partial<ParticleParams>);
    };
  }

  return (
    <div className="w-full h-full relative flex flex-col">
      {/* 窄屏 */}
      {isNarrow && (
        <M3eTabs stretch className="m3e-tabs-compact mb-2 shrink-0">
          <M3eTab
            htmlFor="particle-params"
            selected={activeTab === "params"}
            onClick={() => setActiveTab("params")}
          >
            {t("pages.particle.tabParams")}
          </M3eTab>
          <M3eTab
            htmlFor="particle-palette"
            selected={activeTab === "palette"}
            onClick={() => setActiveTab("palette")}
          >
            {t("pages.particle.tabPalette")}
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
                  {t("pages.particle.argResize")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argResizeDesc")}
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
                  {t("pages.particle.argResizeInterpolation")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argResizeInterpolationDesc")}
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

          {/* 高度 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.particle.argHeight")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argHeightDesc")}
                </span>
              </div>

              <div className="flex gap-4 pt-2">
                <M3eFormField className="w-40 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="height">
                    H
                  </label>
                  <input
                    id="height"
                    type="number"
                    min={0}
                    value={data.height ?? ""}
                    onChange={handleNumberChange("height")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>

          {/* 平面 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.particle.argGenerationPlane")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argGenerationPlaneDesc")}
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

          {/* 模式 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.particle.argMode")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argModeDesc")}
                </span>
              </div>

              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="generation-mode-select"
                  onChange={handleEnumChange("generationMode", MODES)}
                >
                  {MODES.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.generationMode === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 旋转 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.particle.argRotation")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argRotationDesc")}
                </span>
              </div>

              <div className="flex gap-4 pt-2">
                <M3eFormField className="w-24 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="rot-x">
                    X
                  </label>
                  <input
                    id="rot-x"
                    type="number"
                    value={data.rx ?? ""}
                    onChange={handleNumberChange("rx")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField className="w-24 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="rot-y">
                    Y
                  </label>
                  <input
                    id="rot-y"
                    type="number"
                    value={data.ry ?? ""}
                    onChange={handleNumberChange("ry")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>

                <M3eFormField className="w-24 shrink-0" hideSubscript="always">
                  <label slot="label" htmlFor="rot-z">
                    Z
                  </label>
                  <input
                    id="rot-z"
                    type="number"
                    value={data.rz ?? ""}
                    onChange={handleNumberChange("rz")}
                    className="w-full bg-transparent outline-none text-left"
                  />
                </M3eFormField>
              </div>
            </div>
          </div>

          {/* 打包 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0 w-full">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.particle.argPack")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argPackDesc")}
                </span>
              </div>

              <div className="flex flex-col gap-3 pt-2 w-full">
                <M3eFormField
                  className="w-full shrink-0"
                  hideSubscript="always"
                >
                  <label slot="label" htmlFor="pk-name">
                    {t("pages.particle.argPackName")}
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
                    {t("pages.particle.argPackAuth")}
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
                    {t("pages.particle.argPackDescField")}
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

          {/* WS命令间隔 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-start gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.particle.argWebSocketCommandDelay")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.particle.argWebSocketCommandDelayDesc")}
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
        </div>

        {/* 映射表 */}
        <div
          className={
            isNarrow && activeTab !== "palette"
              ? "hidden"
              : "flex flex-1 relative w-full min-h-0"
          }
        >
          <div className="w-full h-full">
            <ParticleMappings />
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
          <M3eFabMenuTrigger htmlFor="particle-fabmenu">
            <M3eIcon name="play_arrow" />
          </M3eFabMenuTrigger>
        </M3eFab>
        <M3eFabMenu id="particle-fabmenu">
          <M3eFabMenuItem onClick={handleFilesClick}>
            <M3eIcon slot="icon" name="file_copy" filled />
            {t("pages.particle.fabGenerateFiles")}
          </M3eFabMenuItem>
          <div
            title={wsRunning ? undefined : t("pages.particle.wsNeedsServer")}
          >
            <M3eFabMenuItem disabled={!wsRunning} onClick={handleWsClick}>
              <M3eIcon slot="icon" name="cable" filled />
              {t("pages.particle.fabGenerateWs")}
            </M3eFabMenuItem>
          </div>
        </M3eFabMenu>
      </div>

      {/* 遮罩 */}
      {processing && (
        <div className="fixed inset-0 z-50 grid place-items-center bg-md-scrim/60">
          <div className="flex items-center gap-6 rounded-md-xl bg-md-surface-container px-8 py-6 shadow-lg">
            {done ? (
              <>
                <M3eIcon
                  name="done_all"
                  filled
                  className="text-3xl text-md-primary"
                />
                <div className="flex max-w-md flex-col gap-1">
                  <span className="text-md-on-surface">{progressText}</span>
                  {wsMode ? (
                    <span className="text-sm text-md-on-surface-variant">
                      {t("pages.particle.wsGoPageHint")}
                    </span>
                  ) : (
                    <>
                      {resultInfo.elapsedMs != null && (
                        <span className="text-sm text-md-on-surface-variant">
                          {t("pages.particle.elapsedLabel")}{" "}
                          {formatElapsed(resultInfo.elapsedMs)}
                        </span>
                      )}
                      {resultInfo.outputDir && (
                        <span className="break-all text-sm text-md-on-surface-variant">
                          {t("pages.particle.outputLabel")}{" "}
                          {resultInfo.outputDir}
                        </span>
                      )}
                    </>
                  )}
                </div>
                {wsMode ? (
                  <>
                    <M3eButton variant="filled" onClick={handleWsGoPage}>
                      {t("pages.particle.wsGoPage")}
                    </M3eButton>
                    <M3eButton variant="tonal" onClick={handleDoneOk}>
                      {t("pages.particle.wsNoThanks")}
                    </M3eButton>
                  </>
                ) : (
                  <M3eButton onClick={handleDoneOk}>OK</M3eButton>
                )}
              </>
            ) : (
              <>
                <M3eCircularProgressIndicator variant="wavy" indeterminate />
                <span className="text-md-on-surface min-w-40">
                  {progressText}
                </span>
                <M3eButton onClick={handleCancel}>取消</M3eButton>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
