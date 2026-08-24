import {
  M3eAppBar,
  M3eDrawerContainer,
  M3eHeading,
  M3eIcon,
  M3eIconButton,
  M3eNavMenu,
  M3eNavMenuItem,
  M3eNavMenuItemGroup,
  M3eTheme,
} from "@m3e/react/all";
import { invoke } from "@tauri-apps/api/core";
import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type TouchEvent as ReactTouchEvent,
} from "react";
import { useTranslation } from "react-i18next";
import "./App.css";
import { useIsNarrow } from "./hooks/useIsNarrow";
import AboutPage from "./pages/AboutPage";
import FaqPage from "./pages/FaqPage";
import ObjToVoxelPage from "./pages/ObjToVoxelPage";
import ParticlePage from "./pages/ParticlePage";
import PixelPage from "./pages/PixelPage";
import SettingsPage, { AppSettings } from "./pages/SettingsPage";
import WebSocketPage from "./pages/WebSocketPage";
import { Storer } from "./stores/storer";

const DEFAULT_SEED_COLOR = "#a8c7fa";
const SEED_SAVE_DEBOUNCE_MS = 300;

interface AppProps {
  initialSettings?: AppSettings;
}

enum Pages {
  Particle = "2dparticle",
  Pixel = "2dblock",
  ObjToVoxel = "3d",
  WebSocket = "websocket",
  Faq = "faq",
  About = "about",
  Settings = "settings",
}

export default function App({ initialSettings }: AppProps = {}) {
  const { t } = useTranslation();
  const [currentTab, setCurrentTab] = useState<Pages>(Pages.Particle);
  const [seedColor, setSeedColor] = useState(
    initialSettings?.seedColor ?? DEFAULT_SEED_COLOR,
  );

  // 960px 响应式布局
  const isNarrow = useIsNarrow();
  const [drawerOpen, setDrawerOpen] = useState(!isNarrow);

  // 安卓文件访问权限检测
  const [downloadPermOk, setDownloadPermOk] = useState(true);
  useEffect(() => {
    if (/Android/i.test(navigator.userAgent)) {
      invoke<boolean>("check_download_permission")
        .then(setDownloadPermOk)
        .catch(() => setDownloadPermOk(true));
    }
  }, []);

  useEffect(() => {
    setDrawerOpen(!isNarrow);
  }, [isNarrow]);

  const handleDrawerChange = (e: Event) => {
    const el = e.currentTarget as unknown as { start?: boolean } | null;
    setDrawerOpen(!!el?.start);
  };

  const handleNav = (p: Pages) => {
    setCurrentTab(p);
    if (isNarrow) setDrawerOpen(false);
  };

  // 右滑手势打开抽屉
  const touchStartX = useRef<number | null>(null);
  const touchStartY = useRef<number | null>(null);
  const handleTouchStart = (e: ReactTouchEvent) => {
    touchStartX.current = e.touches[0]?.clientX ?? null;
    touchStartY.current = e.touches[0]?.clientY ?? null;
  };
  const handleTouchEnd = (e: ReactTouchEvent) => {
    const startX = touchStartX.current;
    const startY = touchStartY.current;
    touchStartX.current = null;
    touchStartY.current = null;
    if (!isNarrow || drawerOpen || startX == null || startY == null) return;
    const t = e.changedTouches[0];
    if (!t) return;
    const dx = t.clientX - startX;
    const dy = t.clientY - startY;
    // 起点在屏幕左边缘 24px 内、向右滑 >50px
    if (startX < 24 && dx > 50 && Math.abs(dy) < dx) {
      setDrawerOpen(true);
    }
  };

  const seedSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingSeedColor = useRef(seedColor);

  const handleSeedColorChange = (color: string) => {
    setSeedColor(color);
    pendingSeedColor.current = color;
    if (seedSaveTimer.current) clearTimeout(seedSaveTimer.current);
    seedSaveTimer.current = setTimeout(() => {
      seedSaveTimer.current = null;
      Storer.savePartial("settings", { seedColor: pendingSeedColor.current });
    }, SEED_SAVE_DEBOUNCE_MS);
  };

  useEffect(() => {
    const flushPendingSave = () => {
      if (seedSaveTimer.current) {
        clearTimeout(seedSaveTimer.current);
        seedSaveTimer.current = null;
        Storer.savePartial("settings", { seedColor: pendingSeedColor.current });
      }
    };
    window.addEventListener("pagehide", flushPendingSave);
    window.addEventListener("beforeunload", flushPendingSave);
    return () => {
      window.removeEventListener("pagehide", flushPendingSave);
      window.removeEventListener("beforeunload", flushPendingSave);
      if (seedSaveTimer.current) clearTimeout(seedSaveTimer.current);
    };
  }, []);

  return (
    <M3eTheme color={seedColor} scheme="dark" motion="standard">
      <div className="h-screen w-screen flex flex-col bg-md-background text-md-on-background overflow-hidden select-none">
        {/* 安全区内边距：edge-to-edge 全面屏下避开状态栏/导航栏/刘海（桌面端 env() 为 0 无影响） */}
        <div
          className="flex-1 flex flex-row overflow-hidden"
          style={{
            paddingTop: "env(safe-area-inset-top)",
            paddingBottom: "env(safe-area-inset-bottom)",
            paddingLeft: "env(safe-area-inset-left)",
            paddingRight: "env(safe-area-inset-right)",
          }}
          onTouchStart={handleTouchStart}
          onTouchEnd={handleTouchEnd}
        >
          {/* Drawer 容器 */}
          <M3eDrawerContainer
            start={drawerOpen}
            startMode="auto"
            startDivider
            onChange={handleDrawerChange}
            className="flex-1 w-full h-full"
            style={{ "--m3e-drawer-container-width": "256px" } as CSSProperties}
          >
            {/* Sidebar（抽屉内容） */}
            <aside
              slot="start"
              className="bg-md-background p-2 flex flex-col gap-4 overflow-y-auto h-full"
            >
              {/* App Title */}
              <div className="flex items-center justify-start px-4 py-3">
                <span className="font-bold text-lg">{t("app.title")}</span>
              </div>

              {/* Nav */}
              <M3eNavMenu>
                <M3eNavMenuItemGroup>
                  <M3eHeading slot="label" variant="label" size="large">
                    {t("app.modeSwitch")}
                  </M3eHeading>

                  <M3eNavMenuItem
                    selected={currentTab === Pages.Particle}
                    onClick={() => handleNav(Pages.Particle)}
                  >
                    <M3eIcon slot="icon" name="bubble_chart" />
                    <span slot="label">{t("app.2dparticle")}</span>
                  </M3eNavMenuItem>

                  <M3eNavMenuItem
                    className="mt-2"
                    selected={currentTab === Pages.Pixel}
                    onClick={() => handleNav(Pages.Pixel)}
                  >
                    <M3eIcon slot="icon" name="grid_view" />
                    <span slot="label">{t("app.2dblock")}</span>
                  </M3eNavMenuItem>

                  <M3eNavMenuItem
                    className="mt-2"
                    selected={currentTab === Pages.ObjToVoxel}
                    onClick={() => handleNav(Pages.ObjToVoxel)}
                  >
                    <M3eIcon slot="icon" name="view_in_ar" />
                    <span slot="label">{t("app.objToVoxel")}</span>
                  </M3eNavMenuItem>

                  <M3eNavMenuItem
                    className="mt-2"
                    selected={currentTab === Pages.WebSocket}
                    onClick={() => handleNav(Pages.WebSocket)}
                  >
                    <M3eIcon slot="icon" name="cable" />
                    <span slot="label">{t("app.websocket")}</span>
                  </M3eNavMenuItem>

                  <M3eNavMenuItem
                    className="mt-2"
                    selected={currentTab === Pages.Faq}
                    onClick={() => handleNav(Pages.Faq)}
                  >
                    <M3eIcon slot="icon" name="help" />
                    <span slot="label">{t("app.faq")}</span>
                  </M3eNavMenuItem>

                  <M3eNavMenuItem
                    className="mt-2"
                    selected={currentTab === Pages.About}
                    onClick={() => handleNav(Pages.About)}
                  >
                    <M3eIcon slot="icon" name="info" />
                    <span slot="label">{t("app.about")}</span>
                  </M3eNavMenuItem>

                  <M3eNavMenuItem
                    className="mt-2"
                    selected={currentTab === Pages.Settings}
                    onClick={() => handleNav(Pages.Settings)}
                  >
                    <M3eIcon slot="icon" name="settings" />
                    <span slot="label">{t("app.settings")}</span>
                  </M3eNavMenuItem>
                </M3eNavMenuItemGroup>
              </M3eNavMenu>
            </aside>

            {/* Workspace */}
            <main className="flex-1 bg-md-background relative overflow-hidden flex flex-col min-w-0 h-full">
              {/* Topbar */}
              {isNarrow && (
                <M3eAppBar size="small">
                  <M3eIconButton
                    slot="leading"
                    aria-label={t("app.title")}
                    onClick={() => setDrawerOpen(true)}
                  >
                    <M3eIcon name="menu" />
                  </M3eIconButton>
                  <span slot="title">{t("app.title")}</span>
                </M3eAppBar>
              )}

              {/* 安卓存储权限未授权提示 */}
              {!downloadPermOk && (
                <div className="flex shrink-0 items-center gap-2 bg-md-error-container px-4 py-2 text-sm text-md-on-error-container">
                  <M3eIcon name="warning" className="shrink-0" />
                  <span>{t("app.storagePermissionHint")}</span>
                </div>
              )}

              <div className="flex-1 relative overflow-hidden min-h-0">
                {currentTab === Pages.Particle && (
                  <ParticlePage
                    onOpenWsPage={() => setCurrentTab(Pages.WebSocket)}
                  />
                )}
                {currentTab === Pages.Pixel && (
                  <PixelPage
                    onOpenWsPage={() => setCurrentTab(Pages.WebSocket)}
                  />
                )}
                {currentTab === Pages.ObjToVoxel && (
                  <ObjToVoxelPage
                    onOpenWsPage={() => setCurrentTab(Pages.WebSocket)}
                  />
                )}
                {currentTab === Pages.WebSocket && <WebSocketPage />}
                {currentTab === Pages.Faq && <FaqPage />}
                {currentTab === Pages.About && <AboutPage />}
                {currentTab === Pages.Settings && (
                  <SettingsPage
                    seedColor={seedColor}
                    onSeedColorChange={handleSeedColorChange}
                  />
                )}
              </div>
            </main>
          </M3eDrawerContainer>
        </div>
      </div>
    </M3eTheme>
  );
}
