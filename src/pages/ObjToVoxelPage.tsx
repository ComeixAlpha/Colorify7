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
import "@m3e/web/button-group";
import "@m3e/web/fab-menu";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ChangeEvent, CSSProperties } from "react";
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/Addons.js";
import bedrockAtlas from "../assets/bedrock_atlas.json";
import bedrockAtlasPngUrl from "../assets/bedrock_atlas.png?url";
import TaskOverlay from "../components/TaskOverlay";
import WorldPicker, { type WorldInfo } from "../components/WorldPicker";
import { useIsNarrow } from "../hooks/useIsNarrow";
import { PageState } from "../session/PageState";
import { Storer } from "../stores/storer";
import { AppSettings } from "./SettingsPage";

export interface ObjParams {
  objPath: string;
  rotation: [number, number, number];
  constraintAxis: string;
  algorithm: string;
  solid: boolean;
  size: number;
  useMultisampleColouring: boolean;
  voxelOverlapRule: string;
  dithering: string;
  ditheringMagnitude: number;
  resolution: number;
  contextualAveraging: boolean;
  errorWeight: number;
  fallable: string;
  useStruct: boolean;
  offsetX: number | null;
  offsetY: number | null;
  offsetZ: number | null;
  gameVersion: string | null;
  useSocket: boolean;
  wsCommandDelay: number;
  useLdb: boolean;
  worldPath: string;
  originX: number | null;
  originY: number | null;
  originZ: number | null;
}

export class ObjToVoxelPageState extends PageState<ObjParams> {
  constructor() {
    super({
      objPath: "",
      rotation: [0, 0, 0],
      constraintAxis: "y",
      algorithm: "triplane",
      solid: true,
      // 3~380
      size: 80,
      useMultisampleColouring: false,
      voxelOverlapRule: "first",
      dithering: "off",
      ditheringMagnitude: 32,
      resolution: 32,
      contextualAveraging: true,
      errorWeight: 0,
      fallable: "replace-fallable",
      useStruct: false,
      offsetX: null,
      offsetY: null,
      offsetZ: null,
      gameVersion: "1.20.80",
      useSocket: false,
      wsCommandDelay: 10,
      useLdb: false,
      worldPath: "",
      originX: null,
      originY: null,
      originZ: null,
    });
  }
}

export const objToVoxelPageState = new ObjToVoxelPageState();

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

interface PreviewBlock {
  name: string;
  x: number;
  y: number;
  z: number;
  colour: [number, number, number];
}

interface PreviewResult {
  blocks: PreviewBlock[];
  min: [number, number, number];
  max: [number, number, number];
}

interface ObjMeshPreview {
  vertices: number[];
  colors: number[];
}

interface WsStatus {
  running: boolean;
  port: number;
  connections: number;
}

interface AtlasJson {
  atlasSize: number;
  cell: number;
  textures: Record<
    string,
    { atlasColumn: number; atlasRow: number; colour: number[]; std: number }
  >;
  blocks: {
    name: string;
    faces: {
      up: string;
      down: string;
      north: string;
      south: string;
      east: string;
      west: string;
    };
  }[];
}

/** 预览用 atlas */
interface AtlasData {
  faces: Map<
    string,
    {
      up: string;
      down: string;
      north: string;
      south: string;
      east: string;
      west: string;
    }
  >;
  texUv: Map<string, { u0: number; v0: number; u1: number; v1: number }>;
}

const CONSTRAINT_AXES = ["x", "y", "z"] as const;
const VOXELISERS = ["triplane", "bvh-ray"] as const;
const OVERLAP_RULES = ["first", "average"] as const;
const DITHERING_MODES = ["off", "random", "ordered"] as const;
const FALLABLE_OPTIONS = [
  "replace-falling",
  "replace-fallable",
  "do-nothing",
] as const;

interface RenderData {
  objMesh: ObjMeshPreview | null;
  preview: PreviewResult | null;
}

let renderData: RenderData = { objMesh: null, preview: null };

const TOOLBAR_ICON_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  lineHeight: 1,
};

export interface VoxelPreviewHandle {
  zoomIn: () => void;
  zoomOut: () => void;
  resetView: () => void;
}

interface VoxelPreviewProps {
  objMesh: ObjMeshPreview | null;
  preview: PreviewResult | null;
  rotation: [number, number, number];
  atlas: AtlasData | null;
  enabled: boolean;
  antialias: boolean;
  showGrid: boolean;
  showAxes: boolean;
}

/**
 * three.js 预览
 */
const VoxelPreview = forwardRef<VoxelPreviewHandle, VoxelPreviewProps>(
  function VoxelPreview(
    {
      objMesh,
      preview,
      rotation,
      atlas,
      enabled,
      antialias,
      showGrid,
      showAxes,
    }: VoxelPreviewProps,
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement>(null);
    const objGroupRef = useRef<THREE.Group | null>(null);
    const renderOnceRef = useRef<(() => void) | null>(null);
    const gridRef = useRef<THREE.GridHelper | null>(null);
    const axesRef = useRef<THREE.AxesHelper | null>(null);
    const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
    const controlsRef = useRef<OrbitControls | null>(null);
    const fitRef = useRef<() => void>(() => {});
    const savedCamRef = useRef<{
      pos: THREE.Vector3;
      target: THREE.Vector3;
      key: string;
    } | null>(null);

    useEffect(() => {
      if (!enabled) return;
      const container = containerRef.current;
      if (!container) return;

      const w = container.clientWidth || 1;
      const h = container.clientHeight || 1;

      // 统一 dispose
      const sceneObjs: { dispose: () => void }[] = [];

      // WebGL 初始化失败保护
      let renderer: THREE.WebGLRenderer;
      try {
        // WebGL MSAA antialias
        renderer = new THREE.WebGLRenderer({ antialias });
      } catch (e) {
        console.error("WebGL 初始化失败:", e);
        return;
      }
      renderer.setPixelRatio(1);
      renderer.setClearColor(0x000000, 0);
      renderer.setSize(w, h);
      container.appendChild(renderer.domElement);

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(50, w / h, 0.1, 1e6);
      const controls = new OrbitControls(camera, renderer.domElement);
      controls.enableDamping = false;

      // 光照
      scene.add(new THREE.AmbientLight(0xffffff, 0.25));
      const key = new THREE.DirectionalLight(0xffffff, 0.9);
      key.position.set(60, 100, 40);
      scene.add(key);
      const fill = new THREE.DirectionalLight(0xffffff, 0.35);
      fill.position.set(-40, -20, -30);
      scene.add(fill);

      const hasVoxels = !!preview && preview.blocks.length > 0;
      const hasObj = !!objMesh && objMesh.vertices.length > 0;

      // 模型包围盒
      const fitMin = new THREE.Vector3(Infinity, Infinity, Infinity);
      const fitMax = new THREE.Vector3(-Infinity, -Infinity, -Infinity);

      if (hasVoxels) {
        fitMin.set(...preview!.min);
        fitMax.set(...preview!.max);
        const center = fitMin.clone().add(fitMax).multiplyScalar(0.5);
        key.target.position.copy(center);
        scene.add(key.target);
        fill.target.position.copy(center);
        scene.add(fill.target);
        const extent = fitMax.clone().sub(fitMin);
        const gridSize = Math.max(extent.x, extent.z, 10);
        const grid = new THREE.GridHelper(
          gridSize,
          Math.min(100, Math.round(gridSize)),
          0x555555,
          0x2a2a2a,
        );
        grid.position.set(center.x, fitMin.y - 1.5, center.z);
        grid.visible = showGrid;
        gridRef.current = grid;
        scene.add(grid);

        const box = new THREE.BoxGeometry(1, 1, 1);
        const boxPos = box.attributes.position.array as Float32Array;
        const boxNrm = box.attributes.normal.array as Float32Array;
        // BoxGeometry 面顺序：0:+x 1:-x 2:+y 3:-y 4:+z 5:-z；每面 4 顶点
        const faceKey = [
          "east",
          "west",
          "up",
          "down",
          "south",
          "north",
        ] as const;
        const faceDir = [
          [1, 0, 0],
          [-1, 0, 0],
          [0, 1, 0],
          [0, -1, 0],
          [0, 0, 1],
          [0, 0, -1],
        ];
        const occupied = new Set<string>();
        for (const b of preview!.blocks) occupied.add(`${b.x},${b.y},${b.z}`);

        // 贴纹理/单色兜底
        const useTex = !!atlas;
        const positions: number[] = [];
        const normals: number[] = [];
        const uvs: number[] = [];
        const colors: number[] = [];
        const indices: number[] = [];
        let vtx = 0;
        for (const b of preview!.blocks) {
          const col = b.colour ?? [0.6, 0.6, 0.6];
          for (let f = 0; f < 6; f++) {
            const d = faceDir[f];
            if (occupied.has(`${b.x + d[0]},${b.y + d[1]},${b.z + d[2]}`))
              continue; // 剔除
            const tex = atlas?.faces.get(b.name)?.[faceKey[f]];
            const uv = tex ? atlas?.texUv.get(tex) : undefined;
            if (useTex && !uv) continue;
            for (let i = 0; i < 4; i++) {
              const vi = f * 4 + i;
              positions.push(
                boxPos[vi * 3] + b.x + 0.5,
                boxPos[vi * 3 + 1] + b.y + 0.5,
                boxPos[vi * 3 + 2] + b.z + 0.5,
              );
              normals.push(
                boxNrm[vi * 3],
                boxNrm[vi * 3 + 1],
                boxNrm[vi * 3 + 2],
              );
              if (useTex) {
                uvs.push(
                  i === 1 || i === 3 ? uv!.u1 : uv!.u0,
                  i === 2 || i === 3 ? uv!.v1 : uv!.v0,
                );
              }
              colors.push(col[0], col[1], col[2]);
            }
            indices.push(vtx, vtx + 2, vtx + 1, vtx + 2, vtx + 3, vtx + 1);
            vtx += 4;
          }
        }
        box.dispose();

        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute(
          "position",
          new THREE.Float32BufferAttribute(positions, 3),
        );
        geometry.setAttribute(
          "normal",
          new THREE.Float32BufferAttribute(normals, 3),
        );
        if (useTex) {
          geometry.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
        }
        geometry.setAttribute(
          "color",
          new THREE.Float32BufferAttribute(colors, 3),
        );
        geometry.setIndex(indices);

        console.log(
          `[3d] 预览：纹理=${useTex} 方块=${preview!.blocks.length} 三角形=${indices.length / 3}`,
        );

        const atlasTex = new THREE.TextureLoader().load(
          bedrockAtlasPngUrl,
          () => renderOnceRef.current?.(),
          undefined,
          () => {
            console.warn("[3d] bedrock atlas 贴图加载失败，降级体素色单色");
            material.map = null;
            material.vertexColors = true;
            material.needsUpdate = true;
            renderOnceRef.current?.();
          },
        );
        atlasTex.magFilter = THREE.NearestFilter;
        atlasTex.minFilter = THREE.NearestFilter;
        atlasTex.flipY = false;
        atlasTex.colorSpace = THREE.SRGBColorSpace;
        const material = new THREE.MeshLambertMaterial(
          useTex
            ? { map: atlasTex, vertexColors: false, alphaTest: 0.5 }
            : { vertexColors: true },
        );
        const mesh = new THREE.Mesh(geometry, material);
        scene.add(mesh);
        sceneObjs.push(geometry, material, atlasTex);
      } else if (hasObj) {
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute(
          "position",
          new THREE.Float32BufferAttribute(objMesh!.vertices, 3),
        );
        geometry.setAttribute(
          "color",
          new THREE.Float32BufferAttribute(objMesh!.colors, 3),
        );
        const material = new THREE.MeshLambertMaterial({
          vertexColors: true,
          side: THREE.DoubleSide,
        });
        const group = new THREE.Group();
        group.add(new THREE.Mesh(geometry, material));
        objGroupRef.current = group;
        scene.add(group);

        const v = objMesh!.vertices;
        for (let i = 0; i < v.length; i += 3) {
          if (v[i] < fitMin.x) fitMin.x = v[i];
          if (v[i] > fitMax.x) fitMax.x = v[i];
          if (v[i + 1] < fitMin.y) fitMin.y = v[i + 1];
          if (v[i + 1] > fitMax.y) fitMax.y = v[i + 1];
          if (v[i + 2] < fitMin.z) fitMin.z = v[i + 2];
          if (v[i + 2] > fitMax.z) fitMax.z = v[i + 2];
        }
        const center = fitMin.clone().add(fitMax).multiplyScalar(0.5);
        key.target.position.copy(center);
        scene.add(key.target);
        fill.target.position.copy(center);
        scene.add(fill.target);
        const gridSize = Math.max(fitMax.x - fitMin.x, fitMax.z - fitMin.z, 10);
        const grid = new THREE.GridHelper(
          gridSize,
          Math.min(100, Math.round(gridSize)),
          0x555555,
          0x2a2a2a,
        );
        grid.position.set(center.x, fitMin.y - 1, center.z);
        grid.visible = showGrid;
        gridRef.current = grid;
        scene.add(grid);
      } else {
        const grid = new THREE.GridHelper(20, 20, 0x555555, 0x2a2a2a);
        grid.position.y = -0.5;
        grid.visible = showGrid;
        gridRef.current = grid;
        scene.add(grid);
      }

      const axes = new THREE.AxesHelper(10);
      axes.visible = showAxes;
      axesRef.current = axes;
      scene.add(axes);

      const fitCamera = () => {
        if (hasVoxels || hasObj) {
          const center = fitMin.clone().add(fitMax).multiplyScalar(0.5);
          const size = fitMax.clone().sub(fitMin).length() || 1;
          camera.position
            .copy(center)
            .add(new THREE.Vector3(size * 1.6, size * 1.2, size * 1.6));
          camera.near = size / 1000;
          camera.far = size * 100;
          camera.updateProjectionMatrix();
          controls.target.copy(center);
          axes.position.copy(center);
          axes.scale.setScalar(Math.max(size / 10, 0.5));
        } else {
          camera.position.set(24, 20, 24);
          camera.lookAt(0, 0, 0);
          axes.position.set(0, 0, 0);
          axes.scale.setScalar(1);
        }
        controls.update();
      };
      const modelKey = preview
        ? `v:${preview.blocks.length}:${preview.min.join(",")}:${preview.max.join(",")}`
        : objMesh
          ? `o:${objMesh.vertices.length}`
          : "empty";
      const saved = savedCamRef.current;
      if (saved && saved.key === modelKey) {
        camera.position.copy(saved.pos);
        controls.target.copy(saved.target);
        controls.update();
      } else {
        fitCamera();
      }
      fitRef.current = fitCamera;
      cameraRef.current = camera;
      controlsRef.current = controls;

      const renderOnce = () => renderer.render(scene, camera);
      renderOnceRef.current = renderOnce;
      controls.addEventListener("change", renderOnce);
      renderOnce();

      const ro = new ResizeObserver(() => {
        const w2 = container.clientWidth || 1;
        const h2 = container.clientHeight || 1;
        renderer.setSize(w2, h2);
        camera.aspect = w2 / h2;
        camera.updateProjectionMatrix();
        renderOnce();
      });
      ro.observe(container);

      return () => {
        savedCamRef.current = {
          pos: camera.position.clone(),
          target: controls.target.clone(),
          key: modelKey,
        };
        ro.disconnect();
        controls.removeEventListener("change", renderOnce);
        controls.dispose();
        for (const o of sceneObjs) o.dispose();
        objGroupRef.current = null;
        renderOnceRef.current = null;
        gridRef.current = null;
        axesRef.current = null;
        cameraRef.current = null;
        controlsRef.current = null;
        fitRef.current = () => {};
        renderer.dispose();
        if (renderer.domElement.parentElement === container) {
          container.removeChild(renderer.domElement);
        }
      };
    }, [preview, objMesh, atlas, enabled, antialias]);

    // 网格 / 坐标轴可见性
    useEffect(() => {
      if (gridRef.current) gridRef.current.visible = showGrid;
      if (axesRef.current) axesRef.current.visible = showAxes;
      renderOnceRef.current?.();
    }, [showGrid, showAxes, enabled]);

    // 放大 / 缩小
    function zoom(factor: number) {
      const camera = cameraRef.current;
      const controls = controlsRef.current;
      if (!camera || !controls) return;
      const dir = camera.position.clone().sub(controls.target);
      const d = dir.length();
      if (d < 1e-3) return;
      camera.position
        .copy(controls.target)
        .addScaledVector(dir.normalize(), d * factor);
      controls.update();
      renderOnceRef.current?.();
    }

    useImperativeHandle(
      ref,
      () => ({
        zoomIn: () => zoom(0.8),
        zoomOut: () => zoom(1.25),
        resetView: () => {
          fitRef.current();
          renderOnceRef.current?.();
        },
      }),
      [],
    );

    useEffect(() => {
      const g = objGroupRef.current;
      if (g) {
        g.rotation.set(
          (rotation[0] * Math.PI) / 180,
          (rotation[1] * Math.PI) / 180,
          (rotation[2] * Math.PI) / 180,
        );
        renderOnceRef.current?.();
      }
    }, [rotation, enabled]);

    return <div ref={containerRef} className="h-full w-full" />;
  },
);

export default function ObjToVoxelPage({
  onOpenWsPage,
}: {
  onOpenWsPage?: () => void;
}) {
  const { t } = useTranslation();
  const { data, update } = objToVoxelPageState.use();
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
  const [render, setRender] = useState<RenderData>(renderData);
  const [atlas, setAtlas] = useState<AtlasData | null>(null);
  const previewApiRef = useRef<VoxelPreviewHandle>(null);
  const [aaOn, setAaOn] = useState(false);
  const [gridOn, setGridOn] = useState(true);
  const [axesOn, setAxesOn] = useState(true);

  const isAndroid = /Android/i.test(navigator.userAgent);

  const isNarrow = useIsNarrow();
  const [activeTab, setActiveTab] = useState<"params" | "preview">("params");
  const toolbarSize: "small" | "medium" = isNarrow ? "medium" : "small";

  const [objData, setObjData] = useState<Uint8Array | null>(null);
  const objInputRef = useRef<HTMLInputElement>(null);

  const fileName = data.objPath
    ? (data.objPath.split(/[\\/]/).pop() ?? data.objPath)
    : "";

  /// 更新渲染数据
  function updateRender(patch: Partial<RenderData>) {
    renderData = { ...renderData, ...patch };
    setRender(renderData);
  }

  useEffect(() => {
    try {
      const json = bedrockAtlas as AtlasJson;
      const rows = Math.ceil(
        Object.keys(json.textures).length / json.atlasSize,
      );
      const atlasW = json.atlasSize * json.cell;
      const atlasH = rows * json.cell;
      const texUv = new Map<
        string,
        { u0: number; v0: number; u1: number; v1: number }
      >();
      for (const [name, t] of Object.entries(json.textures)) {
        const u0 = (t.atlasColumn * json.cell) / atlasW;
        const v0 = (t.atlasRow * json.cell) / atlasH;
        texUv.set(name, {
          u0,
          v0,
          u1: u0 + json.cell / atlasW,
          v1: v0 + json.cell / atlasH,
        });
      }
      const faces = new Map<
        string,
        {
          up: string;
          down: string;
          north: string;
          south: string;
          east: string;
          west: string;
        }
      >();
      for (const b of json.blocks) faces.set(b.name, b.faces);
      setAtlas({ faces, texUv });
      console.log(
        `[3d] atlas 就绪：${json.blocks.length} 方块, ${Object.keys(json.textures).length} 贴图`,
      );
    } catch (e) {
      console.warn("[3d] atlas 解析失败，预览用体素色单色:", e);
    }
  }, []);

  // 轮询服务器状态
  useEffect(() => {
    const id = setInterval(async () => {
      try {
        const s = await invoke<WsStatus>("ws_status");
        setWsRunning(s.running);
      } catch {}
    }, 1000);
    return () => clearInterval(id);
  }, []);

  /// 选择 .obj 文件
  async function handleSelectModel() {
    if (isAndroid) {
      // 安卓：SAF 返回 content:// URI 后端读不了，且 OBJ 依赖同目录 mtl/贴图拿不到；
      // 只支持自包含 GLB：直接读字节传后端
      objInputRef.current?.click();
      return;
    }
    try {
      const path = await open({
        title: t("pages.obj3d.selectModelTitle"),
        filters: [
          { name: "3D Model", extensions: ["obj", "glb", "gltf"] },
          { name: "OBJ", extensions: ["obj"] },
          { name: "glTF", extensions: ["glb", "gltf"] },
        ],
      });
      if (typeof path === "string" && path) {
        update({ objPath: path });
        setProcessing(true);
        setDone(false);
        setWsMode(false);
        setProgressText(t("pages.obj3d.parsingModel"));
        try {
          const m = await invoke<ObjMeshPreview>("get_obj_mesh", { path });
          updateRender({ objMesh: m, preview: null });
        } catch (err) {
          console.error("get_obj_mesh failed:", err);
          updateRender({ objMesh: null, preview: null });
        } finally {
          setProcessing(false);
          setDone(false);
        }
      }
    } catch (err) {
      console.error("open dialog failed:", err);
    }
  }

  async function handleObjFileChange(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0] ?? null;
    e.target.value = "";
    if (!file) return;
    if (!/\.glb$/i.test(file.name) && !/\.gltf$/i.test(file.name)) {
      setProcessing(true);
      setDone(true);
      setWsMode(false);
      setProgressText(t("pages.obj3d.androidGlbOnly"));
      return;
    }
    const bytes = new Uint8Array(await file.arrayBuffer());
    setObjData(bytes);
    update({ objPath: file.name });
    setProcessing(true);
    setDone(false);
    setWsMode(false);
    setProgressText(t("pages.obj3d.parsingModel"));
    try {
      const m = await invoke<ObjMeshPreview>("get_obj_mesh", {
        path: "",
        data: bytes,
      });
      updateRender({ objMesh: m, preview: null });
    } catch (err) {
      console.error("get_obj_mesh failed:", err);
      updateRender({ objMesh: null, preview: null });
      setProgressText(typeof err === "string" ? err : String(err));
    } finally {
      setProcessing(false);
      setDone(false);
    }
  }

  async function handleGenerate(mode: "file" | "socket" | "ldb") {
    if (!data.objPath) {
      await handleSelectModel();
      return;
    }
    modeRef.current = mode;
    cancelledRef.current = false;
    setProcessing(true);
    setDone(false);
    setDoneError(false);
    setWsMode(mode === "socket");
    setProgressText(t("pages.obj3d.preparing"));

    const channel = new Channel<ProgressMessage>();
    channel.onmessage = async (msg) => {
      if (msg.finished) {
        if (cancelledRef.current) return;
        if (msg.error) {
          setDone(true);
          setDoneError(true);
          setProgressText(msg.error);
          return;
        }
        setDone(true);
        setDoneError(false);
        setResultInfo({ elapsedMs: msg.elapsedMs, outputDir: msg.outputDir });
        setProgressText(t("pages.obj3d.taskComplete"));
        if (!isAndroid) {
          try {
            const r = await invoke<PreviewResult | null>("get_obj_result");
            if (r) updateRender({ preview: r });
          } catch {}
        }
      } else {
        setProgressText(msg.stage);
      }
    };

    try {
      await invoke("process_obj", {
        params: {
          ...data,
          objData: isAndroid && objData ? objData : null,
          useSocket: mode === "socket",
          useLdb: mode === "ldb",
          worldPath: mode === "ldb" ? data.worldPath : null,
          originX: mode === "ldb" ? data.originX : null,
          originY: mode === "ldb" ? data.originY : null,
          originZ: mode === "ldb" ? data.originZ : null,
          autoSliceMcfunction:
            Storer.load<AppSettings>("settings").autoSliceMcfunction,
        },
        onProgress: channel,
      });
      console.log("process_obj resolved, 后台线程继续运行");
    } catch (err) {
      console.error("process_obj failed:", err);
      setDone(true);
      setDoneError(true);
      setProgressText(typeof err === "string" ? err : String(err));
    }
  }

  async function handleCancel() {
    cancelledRef.current = true;
    setProcessing(false);
    setDone(false);
    try {
      await invoke("cancel_obj_process");
    } catch (err) {
      console.error("cancel_obj_process failed:", err);
    }
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
        title: t("pages.obj3d.selectWorldTitle"),
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

  function handleDoneOk() {
    setProcessing(false);
    setDone(false);
  }

  function handleWsGoPage() {
    handleDoneOk();
    onOpenWsPage?.();
  }

  function handleNumberChange(field: keyof ObjParams) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      if (raw === "") {
        update({ [field]: null } as Partial<ObjParams>);
        return;
      }
      const n = Number(raw);
      update({ [field]: Number.isNaN(n) ? null : n } as Partial<ObjParams>);
    };
  }

  function handleRequiredNumberChange(field: keyof ObjParams) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const n = Number(e.target.value);
      update({ [field]: Number.isNaN(n) ? 0 : n } as Partial<ObjParams>);
    };
  }

  function handleTextChange(field: keyof ObjParams) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      update({ [field]: e.target.value } as Partial<ObjParams>);
    };
  }

  function handleRotationChange(index: 0 | 1 | 2) {
    return (e: ChangeEvent<HTMLInputElement>) => {
      const n = Number(e.target.value);
      const rot: [number, number, number] = [...data.rotation];
      rot[index] = Number.isNaN(n) ? 0 : n;
      update({ rotation: rot });
    };
  }

  function handleEnumChange<T extends string>(
    field: keyof ObjParams,
    enums: readonly T[],
  ): (e: Event) => void {
    return (e: Event) => {
      const value = (e.target as HTMLSelectElement | null)?.value as T;
      if (!enums.includes(value)) return;
      update({ [field]: value } as Partial<ObjParams>);
    };
  }

  function handleSwitchChange(field: keyof ObjParams): (e: Event) => void {
    return (e: Event) => {
      const checked = (e.target as HTMLInputElement | null)?.checked === true;
      update({ [field]: checked } as Partial<ObjParams>);
    };
  }

  return (
    <div className="h-full w-full relative flex flex-col">
      {/* 窄屏 Tab */}
      {isNarrow && (
        <M3eTabs stretch className="m3e-tabs-compact mb-2 shrink-0">
          <M3eTab
            htmlFor="obj3d-params"
            selected={activeTab === "params"}
            onClick={() => setActiveTab("params")}
          >
            {t("pages.obj3d.tabParams")}
          </M3eTab>
          <M3eTab
            htmlFor="obj3d-preview"
            selected={activeTab === "preview"}
            onClick={() => setActiveTab("preview")}
          >
            {t("pages.obj3d.tabPreview")}
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
              : "flex flex-1 flex-col items-start gap-6 overflow-y-auto scrollbar-thin"
          }
          style={{ justifyContent: "safe center" }}
        >
          {/* 模型文件 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.modelFile")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.modelFileDesc")}
                </span>
              </div>
              <div className="flex items-center gap-3 pt-2">
                <span
                  className={`max-w-64 truncate text-sm ${
                    fileName
                      ? "text-md-on-surface"
                      : "text-md-on-surface-variant"
                  }`}
                >
                  {fileName || t("pages.obj3d.modelFileEmpty")}
                </span>
                <M3eButton variant="tonal" onClick={handleSelectModel}>
                  {t("pages.obj3d.browse")}
                </M3eButton>
              </div>
            </div>
          </div>

          {/* 约束轴 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.obj3d.constraintAxis")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.constraintAxisDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="constraint-axis-select"
                  onChange={handleEnumChange("constraintAxis", CONSTRAINT_AXES)}
                >
                  {CONSTRAINT_AXES.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.constraintAxis === opt}
                    >
                      {opt.toUpperCase()}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 体素化算法 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.obj3d.algorithm")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.algorithmDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="voxeliser-select"
                  onChange={handleEnumChange("algorithm", VOXELISERS)}
                >
                  {VOXELISERS.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.algorithm === opt}
                    >
                      {t(`pages.obj3d.algorithmOptions.${opt}`)}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 实心填充 */}
          {data.algorithm === "triplane" && (
            <div className="flex w-full items-center justify-between gap-4">
              <div className="flex w-full items-center justify-between gap-3">
                <div className="flex min-w-0 flex-col">
                  <span className="font-medium">{t("pages.obj3d.solid")}</span>
                  <span className="text-sm text-md-on-surface-variant">
                    {t("pages.obj3d.solidDesc")}
                  </span>
                </div>
                <M3eSwitch
                  checked={data.solid ?? true}
                  onChange={handleSwitchChange("solid")}
                  className="shrink-0"
                />
              </div>
            </div>
          )}

          {/* 大小 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">{t("pages.obj3d.size")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.sizeDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-40 shrink-0 pt-2"
                hideSubscript="always"
              >
                <label slot="label" htmlFor="obj-size">
                  {t("pages.obj3d.sizeLabel")}
                </label>
                <input
                  id="obj-size"
                  type="number"
                  min={1}
                  value={data.size}
                  onChange={handleRequiredNumberChange("size")}
                  className="w-full bg-transparent text-left outline-none"
                />
              </M3eFormField>
            </div>
          </div>

          {/* 旋转 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">{t("pages.obj3d.rotation")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.rotationDesc")}
                </span>
              </div>
              <div className="flex gap-4 pt-2">
                {([0, 1, 2] as const).map((idx, i) => (
                  <M3eFormField
                    key={idx}
                    className="w-20 shrink-0"
                    hideSubscript="always"
                  >
                    <label slot="label" htmlFor={`obj-rot-${idx}`}>
                      {["X", "Y", "Z"][i]}
                    </label>
                    <input
                      id={`obj-rot-${idx}`}
                      type="number"
                      value={data.rotation[idx]}
                      onChange={handleRotationChange(idx)}
                      className="w-full bg-transparent text-left outline-none"
                    />
                  </M3eFormField>
                ))}
              </div>
            </div>
          </div>

          {/* 多重采样 */}
          <div className="flex w-full items-center justify-between gap-4">
            <div className="flex w-full items-center justify-between gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.multisample")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.multisampleDesc")}
                </span>
              </div>
              <M3eSwitch
                checked={data.useMultisampleColouring}
                onChange={handleSwitchChange("useMultisampleColouring")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 重叠规则 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.obj3d.overlapRule")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.overlapRuleDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="overlap-rule-select"
                  onChange={handleEnumChange("voxelOverlapRule", OVERLAP_RULES)}
                >
                  {OVERLAP_RULES.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.voxelOverlapRule === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 抖动 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">
                  {t("pages.obj3d.dithering")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.ditheringDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="dithering-select"
                  onChange={handleEnumChange("dithering", DITHERING_MODES)}
                >
                  {DITHERING_MODES.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.dithering === opt}
                    >
                      {opt}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 抖动幅度 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.ditheringMagnitude")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.ditheringMagnitudeDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-40 shrink-0 pt-2"
                hideSubscript="always"
              >
                <label slot="label" htmlFor="obj-dither-mag">
                  {t("pages.obj3d.ditheringMagnitudeLabel")}
                </label>
                <input
                  id="obj-dither-mag"
                  type="number"
                  min={0}
                  max={255}
                  value={data.ditheringMagnitude}
                  onChange={handleRequiredNumberChange("ditheringMagnitude")}
                  className="w-full bg-transparent text-left outline-none"
                />
              </M3eFormField>
            </div>
          </div>

          {/* 颜色精度 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.resolution")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.resolutionDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-40 shrink-0 pt-2"
                hideSubscript="always"
              >
                <label slot="label" htmlFor="obj-resolution">
                  {t("pages.obj3d.resolutionLabel")}
                </label>
                <input
                  id="obj-resolution"
                  type="number"
                  min={1}
                  max={255}
                  value={data.resolution}
                  onChange={handleRequiredNumberChange("resolution")}
                  className="w-full bg-transparent text-left outline-none"
                />
              </M3eFormField>
            </div>
          </div>

          {/* 上下文平均 */}
          <div className="flex w-full items-center justify-between gap-4">
            <div className="flex w-full items-center justify-between gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.contextual")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.contextualDesc")}
                </span>
              </div>
              <M3eSwitch
                checked={data.contextualAveraging}
                onChange={handleSwitchChange("contextualAveraging")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 平滑权重 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.errorWeight")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.errorWeightDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-40 shrink-0 pt-2"
                hideSubscript="always"
              >
                <label slot="label" htmlFor="obj-error-weight">
                  {t("pages.obj3d.errorWeightLabel")}
                </label>
                <input
                  id="obj-error-weight"
                  type="number"
                  min={0}
                  max={1}
                  step={0.05}
                  value={data.errorWeight}
                  onChange={handleRequiredNumberChange("errorWeight")}
                  className="w-full bg-transparent text-left outline-none"
                />
              </M3eFormField>
            </div>
          </div>

          {/* 重力方块 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex-col items-center gap-3 min-w-0">
              <div className="flex flex-col min-w-0">
                <span className="font-medium">{t("pages.obj3d.fallable")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.fallableDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-60 shrink-0 pt-2"
                hideSubscript="always"
              >
                <M3eSelect
                  id="fallable-select"
                  onChange={handleEnumChange("fallable", FALLABLE_OPTIONS)}
                >
                  {FALLABLE_OPTIONS.map((opt) => (
                    <M3eOption
                      key={opt}
                      value={opt}
                      selected={data.fallable === opt}
                    >
                      {t(`pages.obj3d.fallableOptions.${opt}`)}
                    </M3eOption>
                  ))}
                </M3eSelect>
              </M3eFormField>
            </div>
          </div>

          {/* 使用结构 */}
          <div className="flex w-full items-center justify-between gap-4">
            <div className="flex w-full items-center justify-between gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.useStruct")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.useStructDesc")}
                </span>
              </div>
              <M3eSwitch
                checked={data.useStruct}
                onChange={handleSwitchChange("useStruct")}
                className="shrink-0"
              />
            </div>
          </div>

          {/* 游戏版本 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">
                  {t("pages.obj3d.gameVersion")}
                </span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.gameVersionDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-40 shrink-0 pt-2"
                hideSubscript="always"
              >
                <label slot="label" htmlFor="obj-game-version">
                  x.x.x
                </label>
                <input
                  id="obj-game-version"
                  type="text"
                  value={data.gameVersion ?? ""}
                  onChange={handleTextChange("gameVersion")}
                  className="w-full bg-transparent text-left outline-none"
                />
              </M3eFormField>
            </div>
          </div>

          {/* 偏移 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">{t("pages.obj3d.offset")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.offsetDesc")}
                </span>
              </div>
              <div className="flex gap-4 pt-2">
                {(["offsetX", "offsetY", "offsetZ"] as const).map(
                  (field, i) => (
                    <M3eFormField
                      key={field}
                      className="w-20 shrink-0"
                      hideSubscript="always"
                    >
                      <label slot="label" htmlFor={`obj-${field}`}>
                        {["X", "Y", "Z"][i]}
                      </label>
                      <input
                        id={`obj-${field}`}
                        type="number"
                        value={data[field] ?? ""}
                        onChange={handleNumberChange(field)}
                        className="w-full bg-transparent text-left outline-none"
                      />
                    </M3eFormField>
                  ),
                )}
              </div>
            </div>
          </div>

          {/* 直写 LevelDB 世界 */}
          <div className="flex w-full items-center justify-between gap-4">
            <div className="flex w-full items-center justify-between gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">{t("pages.obj3d.argLdb")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.argLdbDesc")}
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
                <div className="flex min-w-0 flex-1 flex-col">
                  <span className="font-medium">
                    {t("pages.obj3d.argLdbWorldPath")}
                  </span>
                  <span className="truncate text-sm text-md-on-surface-variant">
                    {data.worldPath ||
                      t("pages.obj3d.argLdbWorldPathPlaceholder")}
                  </span>
                </div>
                <M3eButton
                  variant="tonal"
                  size="small"
                  className="shrink-0"
                  onClick={handleBrowseWorld}
                >
                  {t("pages.obj3d.argLdbBrowse")}
                </M3eButton>
              </div>
              {/* 生成坐标 */}
              <div className="flex flex-col gap-2">
                <span className="font-medium">
                  {t("pages.obj3d.argLdbOrigin")}
                </span>
                <div className="flex w-full min-w-0 gap-2">
                  {(["originX", "originY", "originZ"] as const).map(
                    (field, i) => (
                      <M3eFormField
                        key={field}
                        className="flex-1 shrink-0"
                        hideSubscript="always"
                      >
                        <label slot="label" htmlFor={`obj-ldb-${field}`}>
                          {["X", "Y", "Z"][i]}
                        </label>
                        <input
                          id={`obj-ldb-${field}`}
                          type="number"
                          value={data[field] ?? ""}
                          onChange={handleNumberChange(field)}
                          className="w-full bg-transparent text-left outline-none"
                        />
                      </M3eFormField>
                    ),
                  )}
                </div>
              </div>
            </div>
          )}

          {/* WS 命令延迟 */}
          <div className="flex items-center justify-between gap-4">
            <div className="flex min-w-0 flex-col items-start gap-3">
              <div className="flex min-w-0 flex-col">
                <span className="font-medium">{t("pages.obj3d.wsDelay")}</span>
                <span className="text-sm text-md-on-surface-variant">
                  {t("pages.obj3d.wsDelayDesc")}
                </span>
              </div>
              <M3eFormField
                className="w-40 shrink-0 pt-2"
                hideSubscript="always"
              >
                <label slot="label" htmlFor="obj-ws-delay">
                  Delay
                </label>
                <input
                  id="obj-ws-delay"
                  type="number"
                  min={1}
                  value={data.wsCommandDelay}
                  onChange={handleRequiredNumberChange("wsCommandDelay")}
                  className="w-full bg-transparent text-left outline-none"
                />
              </M3eFormField>
            </div>
          </div>
        </div>

        {/* 预览区 + 悬浮视图工具栏 */}
        <div
          className={
            isNarrow && activeTab !== "preview"
              ? "hidden"
              : "relative flex flex-2 w-full overflow-hidden rounded-md-xl border border-md-outline-variant/40 min-h-0"
          }
        >
          <div className="bg-md-surface-container/50 h-full w-full">
            <VoxelPreview
              ref={previewApiRef}
              objMesh={render.objMesh}
              preview={render.preview}
              rotation={data.rotation}
              atlas={atlas}
              enabled={!isAndroid}
              antialias={aaOn}
              showGrid={gridOn}
              showAxes={axesOn}
            />
          </div>

          {isAndroid && (
            <div className="absolute inset-0 grid place-items-center p-8 text-center text-sm text-md-on-surface-variant">
              {t("pages.obj3d.androidNoPreview")}
            </div>
          )}
          {!isAndroid && !render.objMesh && !render.preview && (
            <div className="pointer-events-none absolute inset-0 grid place-items-center p-8 text-center text-sm text-md-on-surface-variant">
              {t("pages.obj3d.previewEmpty")}
            </div>
          )}

          {/* 视图工具栏 */}
          {!isAndroid && (
            <div
              className={
                isNarrow
                  ? "absolute bottom-3 inset-x-3"
                  : "absolute bottom-3 left-1/2 -translate-x-1/2"
              }
            >
              <div
                className={
                  isNarrow
                    ? "toolbar-stretch flex items-center gap-2 w-full"
                    : "flex items-center gap-2"
                }
                style={
                  { "--m3e-button-icon-label-space": "0px" } as CSSProperties
                }
              >
                <M3eButton
                  variant="tonal"
                  size={toolbarSize}
                  toggle
                  selected={aaOn}
                  onChange={() => setAaOn((v) => !v)}
                  title={t("pages.obj3d.toolbar.antialias")}
                >
                  <M3eIcon
                    slot="icon"
                    name={aaOn ? "blur_on" : "blur_off"}
                    style={TOOLBAR_ICON_STYLE}
                  />
                </M3eButton>
                <M3eButton
                  variant="tonal"
                  size={toolbarSize}
                  toggle
                  selected={gridOn}
                  onChange={() => setGridOn((v) => !v)}
                  title={t("pages.obj3d.toolbar.grid")}
                >
                  <M3eIcon
                    slot="icon"
                    name={gridOn ? "grid_on" : "grid_off"}
                    style={TOOLBAR_ICON_STYLE}
                  />
                </M3eButton>
                <M3eButton
                  variant="tonal"
                  size={toolbarSize}
                  toggle
                  selected={axesOn}
                  onChange={() => setAxesOn((v) => !v)}
                  title={t("pages.obj3d.toolbar.axes")}
                >
                  <M3eIcon
                    slot="icon"
                    name={axesOn ? "explore" : "explore_off"}
                    style={TOOLBAR_ICON_STYLE}
                  />
                </M3eButton>
                <M3eButton
                  variant="tonal"
                  size={toolbarSize}
                  onClick={() => previewApiRef.current?.zoomIn()}
                  title={t("pages.obj3d.toolbar.zoomIn")}
                >
                  <M3eIcon
                    slot="icon"
                    name="zoom_in_map"
                    style={TOOLBAR_ICON_STYLE}
                  />
                </M3eButton>
                <M3eButton
                  variant="tonal"
                  size={toolbarSize}
                  onClick={() => previewApiRef.current?.zoomOut()}
                  title={t("pages.obj3d.toolbar.zoomOut")}
                >
                  <M3eIcon
                    slot="icon"
                    name="zoom_out_map"
                    style={TOOLBAR_ICON_STYLE}
                  />
                </M3eButton>
                <M3eButton
                  variant="tonal"
                  size={toolbarSize}
                  onClick={() => previewApiRef.current?.resetView()}
                  title={t("pages.obj3d.toolbar.resetView")}
                >
                  <M3eIcon
                    slot="icon"
                    name="center_focus_strong"
                    style={TOOLBAR_ICON_STYLE}
                  />
                </M3eButton>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* FAB */}
      <div
        className={
          isNarrow && activeTab === "preview"
            ? "hidden"
            : "absolute right-2 bottom-2"
        }
      >
        <input
          ref={objInputRef}
          type="file"
          accept=".glb,.gltf"
          className="hidden"
          onChange={handleObjFileChange}
        />
        <M3eFab variant="primary" size="medium">
          <M3eFabMenuTrigger htmlFor="obj3d-fabmenu">
            <M3eIcon name="play_arrow" />
          </M3eFabMenuTrigger>
        </M3eFab>
        <M3eFabMenu id="obj3d-fabmenu">
          <M3eFabMenuItem onClick={handleSelectModel}>
            <M3eIcon slot="icon" name="folder_open" filled />
            {t("pages.obj3d.selectModelAction")}
          </M3eFabMenuItem>
          <M3eFabMenuItem
            disabled={!data.objPath}
            onClick={() => handleGenerate("file")}
          >
            <M3eIcon slot="icon" name="file_copy" filled />
            {t("pages.obj3d.generateFiles")}
          </M3eFabMenuItem>
          <div title={wsRunning ? undefined : t("pages.obj3d.wsNeedsServer")}>
            <M3eFabMenuItem
              disabled={!data.objPath || !wsRunning}
              onClick={() => handleGenerate("socket")}
            >
              <M3eIcon slot="icon" name="cable" filled />
              {t("pages.obj3d.generateWs")}
            </M3eFabMenuItem>
          </div>
          <div
            title={
              data.useLdb && data.worldPath
                ? undefined
                : t("pages.obj3d.ldbNeedsWorld")
            }
          >
            <M3eFabMenuItem
              disabled={!data.objPath || !data.useLdb || !data.worldPath}
              onClick={() => handleGenerate("ldb")}
            >
              <M3eIcon slot="icon" name="storage" filled />
              {t("pages.obj3d.generateLdb")}
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
        wsHint={t("pages.obj3d.wsGoPageHint")}
        wsGoPageLabel={t("pages.obj3d.wsGoPage")}
        wsNoThanksLabel={t("pages.obj3d.wsNoThanks")}
        elapsedLabel={t("pages.obj3d.elapsedLabel")}
        outputLabel={t("pages.obj3d.outputLabel")}
        cancelLabel={t("pages.obj3d.cancel")}
        onCancel={handleCancel}
        onDoneOk={handleDoneOk}
        onWsGoPage={handleWsGoPage}
      />

      <WorldPicker
        open={pickerOpen}
        worlds={worldChoices}
        loading={pickerLoading}
        error={pickerError ?? undefined}
        title={t("pages.obj3d.selectWorldTitle")}
        emptyText={t("pages.obj3d.argLdbNoWorld")}
        hint={t("pages.obj3d.argLdbWorldDirHint")}
        onOpenSettings={
          isAndroid
            ? () => {
                invoke("open_all_files_settings").catch((err) =>
                  console.error("open settings failed:", err),
                );
              }
            : undefined
        }
        settingsLabel={t("pages.obj3d.argLdbOpenSettings")}
        onSelect={(path) => {
          update({ worldPath: path });
          setPickerOpen(false);
        }}
        onClose={() => setPickerOpen(false)}
      />
    </div>
  );
}
