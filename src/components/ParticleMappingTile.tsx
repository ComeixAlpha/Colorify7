import { M3eButton, M3eIcon } from "@m3e/react/all";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { ParticleMapping } from "../pages/ParticleMappings";

interface ParticleMappingTileProps {
  mapping: ParticleMapping;
  onDelete: () => void;
}

export default function ParticleMappingTile({
  mapping,
  onDelete,
}: ParticleMappingTileProps) {
  const { t } = useTranslation();
  const color = `rgb(${mapping.r}, ${mapping.g}, ${mapping.b})`;

  return (
    <div className="w-full flex items-center justify-between gap-4 rounded-md-xl bg-md-surface-container px-4 py-3">
      <div className="flex items-center gap-4 min-w-0">
        {/* 色块 */}
        <div
          className="w-14 h-14 rounded-full shrink-0 grid place-items-center"
          style={{ backgroundColor: color }}
        >
          <M3eIcon name="auto_awesome" filled className="text-white" />
        </div>
        <div className="flex flex-col min-w-0">
          <span className="text-md-on-surface">
            {t("pages.particle.mappingsRgb")} ({mapping.r}, {mapping.g},{" "}
            {mapping.b})
          </span>
          <span className="text-sm text-md-on-surface-variant truncate">
            {mapping.id}
          </span>
        </div>
      </div>

      <M3eButton
        variant="tonal"
        className="shrink-0"
        style={{ "--m3e-button-icon-label-space": "0px" } as CSSProperties}
        onClick={onDelete}
        title={t("pages.particle.mappingsDelete")}
      >
        <M3eIcon slot="icon" name="close" />
      </M3eButton>
    </div>
  );
}
