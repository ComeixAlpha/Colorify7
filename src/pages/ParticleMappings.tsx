import { M3eButton, M3eFormField, M3eIcon } from "@m3e/react/all";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import ParticleMappingTile from "../components/ParticleMappingTile";
import { Storer } from "../stores/storer";

export interface ParticleMapping {
  r: number;
  g: number;
  b: number;
  id: string;
}

export interface ParticleMappingsProps {
  mappings: ParticleMapping[];
}

const DEFAULT_PARTICLE_MAPPINGS_PROPS: ParticleMappingsProps = {
  mappings: [{ r: 255, g: 255, b: 255, id: "minecraft:endrod" }],
};

Storer.registerDefaults(
  "particle_mappings_props",
  DEFAULT_PARTICLE_MAPPINGS_PROPS,
);

export async function loadParticleMappings(): Promise<ParticleMapping[]> {
  const props = await Storer.loadPref<ParticleMappingsProps>(
    "particle_mappings_props",
  );
  return props.mappings.length > 0
    ? props.mappings
    : DEFAULT_PARTICLE_MAPPINGS_PROPS.mappings;
}

export default function ParticleMappings() {
  const { t } = useTranslation();
  const [mappings, setMappings] = useState<ParticleMapping[]>(
    DEFAULT_PARTICLE_MAPPINGS_PROPS.mappings,
  );
  const [showNew, setShowNew] = useState(false);

  useEffect(() => {
    loadParticleMappings().then((m) => setMappings(m));
  }, []);

  function persist(next: ParticleMapping[]) {
    setMappings(next);
    Storer.savePartial("particle_mappings_props", { mappings: next });
  }

  function handleDelete(index: number) {
    persist(mappings.filter((_, i) => i !== index));
  }

  function handleAdd(m: ParticleMapping) {
    persist([...mappings, m]);
    setShowNew(false);
  }

  return (
    <div className="w-full h-full flex flex-col gap-4">
      {/* 标题 + 新建按钮 */}
      <div className="flex w-full items-center justify-between gap-4">
        <div className="flex flex-col min-w-0">
          <span className="font-medium">{t("pages.particle.mappings")}</span>
          <span className="text-sm text-md-on-surface-variant">
            {t("pages.particle.mappingsDesc")}
          </span>
        </div>
        <M3eButton variant="tonal" onClick={() => setShowNew(true)}>
          <M3eIcon slot="icon" name="add" />
          {t("pages.particle.mappingsNew")}
        </M3eButton>
      </div>

      {/* 映射列表 */}
      <div
        className="flex flex-1 flex-col min-h-0 gap-3 items-start overflow-y-auto scrollbar-thin"
        style={{ justifyContent: "safe center" }}
      >
        {mappings.length === 0 ? (
          <div className="flex flex-col items-center gap-4 w-full py-10">
            <span className="text-md-on-surface-variant">
              {t("pages.particle.mappingsEmpty")}
            </span>
            <M3eButton variant="filled" onClick={() => setShowNew(true)}>
              <M3eIcon slot="icon" name="add" />
              {t("pages.particle.mappingsNew")}
            </M3eButton>
          </div>
        ) : (
          mappings.map((m, i) => (
            <ParticleMappingTile
              key={`${m.id}-${i}`}
              mapping={m}
              onDelete={() => handleDelete(i)}
            />
          ))
        )}
      </div>

      {showNew && (
        <NewParticleMappingDialog
          onDone={handleAdd}
          onCancel={() => setShowNew(false)}
        />
      )}
    </div>
  );
}

interface NewParticleMappingDialogProps {
  onDone: (m: ParticleMapping) => void;
  onCancel: () => void;
}

function NewParticleMappingDialog({
  onDone,
  onCancel,
}: NewParticleMappingDialogProps) {
  const { t } = useTranslation();
  const [r, setR] = useState("");
  const [g, setG] = useState("");
  const [b, setB] = useState("");
  const [id, setId] = useState("");
  const [error, setError] = useState<string | null>(null);

  function parseChannel(v: string): number | null {
    if (v.trim() === "") return null;
    const n = Number(v.trim());
    if (!Number.isInteger(n) || n < 0 || n > 255) return null;
    return n;
  }

  function handleConfirm() {
    const rv = parseChannel(r);
    const gv = parseChannel(g);
    const bv = parseChannel(b);
    const idv = id.trim();
    if (rv === null || gv === null || bv === null) {
      setError(t("pages.particle.mappingsErrorRgb"));
      return;
    }
    if (idv === "") {
      setError(t("pages.particle.mappingsErrorId"));
      return;
    }
    onDone({ r: rv, g: gv, b: bv, id: idv });
  }

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-md-scrim/60"
      onClick={onCancel}
    >
      <div
        className="flex w-96 flex-col gap-4 rounded-md-xl bg-md-surface-container px-6 py-6 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <span className="text-lg text-md-on-surface">
          {t("pages.particle.mappingsNewTitle")}
        </span>

        <div className="flex gap-3">
          <M3eFormField className="flex-1" hideSubscript="always">
            <label slot="label" htmlFor="map-r">
              R
            </label>
            <input
              id="map-r"
              type="number"
              min={0}
              max={255}
              value={r}
              onChange={(e) => setR(e.target.value)}
              className="w-full bg-transparent outline-none text-left"
            />
          </M3eFormField>
          <M3eFormField className="flex-1" hideSubscript="always">
            <label slot="label" htmlFor="map-g">
              G
            </label>
            <input
              id="map-g"
              type="number"
              min={0}
              max={255}
              value={g}
              onChange={(e) => setG(e.target.value)}
              className="w-full bg-transparent outline-none text-left"
            />
          </M3eFormField>
          <M3eFormField className="flex-1" hideSubscript="always">
            <label slot="label" htmlFor="map-b">
              B
            </label>
            <input
              id="map-b"
              type="number"
              min={0}
              max={255}
              value={b}
              onChange={(e) => setB(e.target.value)}
              className="w-full bg-transparent outline-none text-left"
            />
          </M3eFormField>
        </div>

        <M3eFormField className="w-full" hideSubscript="always">
          <label slot="label" htmlFor="map-id">
            {t("pages.particle.mappingsPid")}
          </label>
          <input
            id="map-id"
            type="text"
            placeholder="minecraft:endrod"
            value={id}
            onChange={(e) => setId(e.target.value)}
            className="w-full bg-transparent outline-none text-left"
          />
        </M3eFormField>

        {error && <span className="text-sm text-md-error">{error}</span>}

        <div className="flex items-center justify-end gap-3">
          <M3eButton variant="text" onClick={onCancel}>
            取消
          </M3eButton>
          <M3eButton variant="filled" onClick={handleConfirm}>
            确定
          </M3eButton>
        </div>
      </div>
    </div>
  );
}
