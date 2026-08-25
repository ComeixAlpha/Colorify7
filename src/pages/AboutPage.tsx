import { M3eAvatar, M3eHeading, M3eIcon } from "@m3e/react/all";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import authorAvatar from "../assets/my_avatar.jpg";

const LEGACY_DOC_URL = "https://comeixalpha.github.io/";
const GITHUB_URL = "https://github.com/ComeixAlpha/Colorify7";
const SLOPE_CRAFT_URL = "https://github.com/SlopeCraft/SlopeCraft";
const AUTHOR_EMAIL = "omeixc@gmail.com";
const AUTHOR_QQ = "3695178121";
const OBJ_TO_SCHEMATIC_URL = "https://github.com/LucasDower/ObjToSchematic";
const MOJANG_GUIDELINES_URL = "https://account.mojang.com/terms#brand";

interface AboutItem {
  key: string;
  icon: string;
}

const FEATURES: AboutItem[] = [
  { key: "featureParticle", icon: "bubble_chart" },
  { key: "featurePixel", icon: "grid_view" },
  { key: "featureVoxel", icon: "view_in_ar" },
  { key: "featureWebSocket", icon: "cable" },
  { key: "featureLdb", icon: "database" },
];

const AVATAR_URLS = import.meta.glob<string>(
  "../assets/acknowledgements/*.{jpg,jpeg,png}",
  { eager: true, import: "default" },
);

const CONTRIBUTORS: { name: string; src: string }[] = [
  {
    name: "pages.special_thanks.quietfallhe",
    src: "../assets/acknowledgements/quietfallhe.jpg",
  },
  {
    name: "pages.special_thanks.els",
    src: "../assets/acknowledgements/els.jpg",
  },
  {
    name: "pages.special_thanks.tokinobug",
    src: "../assets/acknowledgements/tokinobug.jpg",
  },
  {
    name: "pages.special_thanks.dislink",
    src: "../assets/acknowledgements/dislink.jpg",
  },
  {
    name: "pages.special_thanks.deltard",
    src: "../assets/acknowledgements/deltard.jpg",
  },
  {
    name: "pages.special_thanks.glaze",
    src: "../assets/acknowledgements/glaze.jpg",
  },
  {
    name: "pages.special_thanks.wn1027",
    src: "../assets/acknowledgements/wn1027.jpg",
  },
  {
    name: "pages.special_thanks.happy",
    src: "../assets/acknowledgements/happy.jpg",
  },
  {
    name: "pages.special_thanks.projectxero",
    src: "../assets/acknowledgements/projectxero.png",
  },
  {
    name: "pages.special_thanks.nuclear",
    src: "../assets/acknowledgements/nuclear.png",
  },
  {
    name: "pages.special_thanks.hcdyx",
    src: "../assets/acknowledgements/hcdyx.jpg",
  },
];

const CONTRIBUTOR_ITEMS = CONTRIBUTORS.map(({ name, src }) => ({
  name,
  src: AVATAR_URLS[src],
}));

export default function AboutPage() {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");

  useEffect(() => {
    let cancelled = false;
    void getVersion()
      .then((v) => {
        if (!cancelled) setVersion(v);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const openLink = (url: string) => () => {
    void openUrl(url).catch(() => {
      window.open(url, "_blank", "noopener,noreferrer");
    });
  };

  const [copied, setCopied] = useState<"email" | "qq" | null>(null);
  const copyContact = (key: "email" | "qq", value: string) => {
    void navigator.clipboard
      .writeText(value)
      .then(() => {
        setCopied(key);
        window.setTimeout(() => setCopied((k) => (k === key ? null : k)), 1500);
      })
      .catch(() => {});
  };

  return (
    <div className="h-full w-full overflow-y-auto p-6 scrollbar-thin">
      <div className="flex flex-col gap-6">
        {/* Hero：应用标识 */}
        <div className="flex items-center gap-5 rounded-md-xl bg-md-surface-container px-6 py-5">
          <M3eAvatar
            style={
              {
                "--m3e-avatar-size": "72px",
                "--m3e-avatar-shape": "22px",
                "--m3e-avatar-color": "var(--md-sys-color-primary-container)",
                "--m3e-avatar-label-color":
                  "var(--md-sys-color-on-primary-container)",
              } as CSSProperties
            }
          >
            {/* 应用图标（与桌面/安卓同源，来自 src-tauri/icons/icon.png） */}
            <img
              src="/app-icon.png"
              alt={t("app.title")}
              className="h-full w-full object-cover"
            />
          </M3eAvatar>
          <div className="flex min-w-0 flex-col gap-0.5">
            <M3eHeading variant="headline" size="small">
              {t("app.title")}
            </M3eHeading>
            <span className="text-sm text-md-on-surface-variant">
              {t("app.badge")} · {t("app.platform")}
              {version && ` · v${version}`}
            </span>
          </div>
        </div>

        {/* 简介 */}
        <p className="px-1 text-sm leading-relaxed text-md-on-surface-variant">
          {t("pages.about.description")}
        </p>

        {/* 作者与联系方式 */}
        <section className="flex flex-col gap-3">
          <M3eHeading
            variant="label"
            size="large"
            className="px-1 text-md-on-surface-variant"
          >
            {t("pages.about.author")}
          </M3eHeading>
          <div className="flex items-center gap-4 rounded-md-xl bg-md-surface-container px-5 py-4">
            <img
              src={authorAvatar}
              alt={t("pages.about.authorName")}
              className="h-14 w-14 shrink-0 rounded-full object-cover ring-1 ring-md-outline-variant"
            />
            <div className="flex min-w-0 flex-1 flex-col gap-2">
              <span className="font-semibold">
                {t("pages.about.authorName")}
              </span>
              <div className="flex flex-col gap-1.5 text-xs text-md-on-surface-variant">
                {/* Email */}
                <div className="flex items-center gap-2">
                  <M3eIcon
                    name="mail"
                    className="shrink-0 text-base text-md-primary"
                  />
                  <span className="truncate">
                    {t("pages.about.emailLabel")}: {AUTHOR_EMAIL}
                  </span>
                  <button
                    type="button"
                    onClick={() => copyContact("email", AUTHOR_EMAIL)}
                    aria-label={t("pages.about.copy")}
                    className="ml-auto shrink-0 rounded-full p-1 text-md-on-surface-variant transition-colors hover:bg-md-surface-container-high hover:text-md-on-surface"
                  >
                    <M3eIcon
                      name={copied === "email" ? "check" : "content_copy"}
                      className="text-sm"
                    />
                  </button>
                </div>
                {/* QQ */}
                <div className="flex items-center gap-2">
                  <M3eIcon
                    name="chat"
                    className="shrink-0 text-base text-md-primary"
                  />
                  <span className="truncate">
                    {t("pages.about.qqLabel")}: {AUTHOR_QQ}
                  </span>
                  <button
                    type="button"
                    onClick={() => copyContact("qq", AUTHOR_QQ)}
                    aria-label={t("pages.about.copy")}
                    className="ml-auto shrink-0 rounded-full p-1 text-md-on-surface-variant transition-colors hover:bg-md-surface-container-high hover:text-md-on-surface"
                  >
                    <M3eIcon
                      name={copied === "qq" ? "check" : "content_copy"}
                      className="text-sm"
                    />
                  </button>
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 功能特性 */}
        <section className="flex flex-col gap-3">
          <M3eHeading
            variant="label"
            size="large"
            className="px-1 text-md-on-surface-variant"
          >
            {t("pages.about.features")}
          </M3eHeading>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {FEATURES.map((f) => (
              <div
                key={f.key}
                className="flex items-center gap-3 rounded-md-xl bg-md-surface-container py-3 pl-5 pr-4"
              >
                <M3eIcon
                  name={f.icon}
                  className="shrink-0 text-xl text-md-primary"
                />
                <div className="flex min-w-0 flex-col">
                  <span className="text-sm font-medium">
                    {t(`pages.about.${f.key}`)}
                  </span>
                  <span className="text-xs leading-relaxed text-md-on-surface-variant">
                    {t(`pages.about.${f.key}Desc`)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* 相关链接 */}
        <section className="flex flex-col gap-3">
          <M3eHeading
            variant="label"
            size="large"
            className="px-1 text-md-on-surface-variant"
          >
            {t("pages.about.links")}
          </M3eHeading>
          <div className="flex flex-col gap-2">
            <button
              type="button"
              onClick={openLink(GITHUB_URL)}
              className="flex cursor-pointer items-center gap-4 rounded-md-xl bg-md-surface-container px-5 py-4 text-left transition-colors hover:bg-md-surface-container-high"
            >
              <M3eIcon
                name="code"
                className="shrink-0 text-2xl text-md-primary"
              />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="font-medium">{t("pages.about.github")}</span>
                <span className="truncate text-xs text-md-on-surface-variant">
                  {t("pages.about.githubDesc")}
                </span>
              </div>
              <M3eIcon
                name="open_in_new"
                className="shrink-0 text-md-on-surface-variant"
              />
            </button>

            <button
              type="button"
              onClick={openLink(LEGACY_DOC_URL)}
              className="flex cursor-pointer items-center gap-4 rounded-md-xl bg-md-surface-container px-5 py-4 text-left transition-colors hover:bg-md-surface-container-high"
            >
              <M3eIcon
                name="code"
                className="shrink-0 text-2xl text-md-primary"
              />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="font-medium">
                  {t("pages.about.legacyDoc")}
                </span>
                <span className="truncate text-xs text-md-on-surface-variant">
                  {t("pages.about.legacyDocDesc")}
                </span>
              </div>
              <M3eIcon
                name="open_in_new"
                className="shrink-0 text-md-on-surface-variant"
              />
            </button>
          </div>
        </section>

        {/* 致谢 */}
        <section className="flex flex-col gap-3">
          <M3eHeading
            variant="label"
            size="large"
            className="px-1 text-md-on-surface-variant"
          >
            {t("pages.about.thirdParty")}
          </M3eHeading>

          {/* 特别感谢：贡献者头像网格 */}
          <M3eHeading
            variant="label"
            size="medium"
            className="px-1 text-md-on-surface-variant"
          >
            {t("pages.about.specialThanks")}
          </M3eHeading>
          <div className="grid grid-cols-4 gap-2 sm:grid-cols-5 md:grid-cols-6 xl:grid-cols-8">
            {CONTRIBUTOR_ITEMS.map((c) => (
              <div
                key={c.src}
                className="flex flex-col items-center gap-2 rounded-md-xl bg-md-surface-container px-1 py-3"
              >
                <img
                  src={c.src}
                  alt={t(c.name)}
                  className="h-11 w-11 rounded-full object-cover ring-1 ring-md-outline-variant"
                />
                <span className="max-w-full truncate text-xs text-md-on-surface-variant">
                  {t(c.name)}
                </span>
              </div>
            ))}
          </div>

          <div className="flex flex-col gap-2">
            <button
              type="button"
              onClick={openLink(SLOPE_CRAFT_URL)}
              className="flex cursor-pointer items-center gap-4 rounded-md-xl bg-md-surface-container px-5 py-4 text-left transition-colors hover:bg-md-surface-container-high"
            >
              <M3eIcon
                name="memory"
                className="shrink-0 text-2xl text-md-tertiary"
              />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="font-medium">
                  {t("pages.about.slopeCraft")}
                </span>
                <span className="truncate text-xs text-md-on-surface-variant">
                  {t("pages.about.slopeCraftDesc")}
                </span>
              </div>
              <M3eIcon
                name="open_in_new"
                className="shrink-0 text-md-on-surface-variant"
              />
            </button>

            <button
              type="button"
              onClick={openLink(OBJ_TO_SCHEMATIC_URL)}
              className="flex cursor-pointer items-center gap-4 rounded-md-xl bg-md-surface-container px-5 py-4 text-left transition-colors hover:bg-md-surface-container-high"
            >
              <M3eIcon
                name="memory"
                className="shrink-0 text-2xl text-md-tertiary"
              />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="font-medium">
                  {t("pages.about.objToSchematic")}
                </span>
                <span className="truncate text-xs text-md-on-surface-variant">
                  {t("pages.about.objToSchematicDesc")}
                </span>
              </div>
              <M3eIcon
                name="open_in_new"
                className="shrink-0 text-md-on-surface-variant"
              />
            </button>

            <button
              type="button"
              onClick={openLink(MOJANG_GUIDELINES_URL)}
              className="flex cursor-pointer items-center gap-4 rounded-md-xl bg-md-surface-container px-5 py-4 text-left transition-colors hover:bg-md-surface-container-high"
            >
              <M3eIcon
                name="crop_square"
                className="shrink-0 text-2xl text-md-tertiary"
              />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="font-medium">{t("pages.about.mojang")}</span>
                <span className="truncate text-xs text-md-on-surface-variant">
                  {t("pages.about.mojangDesc")}
                </span>
              </div>
              <M3eIcon
                name="open_in_new"
                className="shrink-0 text-md-on-surface-variant"
              />
            </button>
          </div>
        </section>

        {/* 版权声明 */}
        <p className="px-1 pb-4 text-left text-xs leading-relaxed text-md-on-surface-variant/70">
          {t("pages.about.copyright")}
        </p>
      </div>
    </div>
  );
}
