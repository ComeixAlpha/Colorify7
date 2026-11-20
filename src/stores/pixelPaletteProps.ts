import { Storer } from "./storer";

/**
 * 像素画调色板偏好（禁用方块集合）
 */

/** 键名 */
export const PIXEL_PALETTE_PROPS_KEY = "pixel_palette_props";

export const PIXEL_PALETTE_PROPS_CHANGED_EVENT =
  "colorify:palette-props-changed";

export interface PixelPaletteProps {
  disabledIds: string[];
}

export const DEFAULT_PIXEL_PALETTE_PROPS: PixelPaletteProps = {
  disabledIds: ["tnt"],
};

Storer.registerDefaults(PIXEL_PALETTE_PROPS_KEY, DEFAULT_PIXEL_PALETTE_PROPS);

function normalizeDisabledIds(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [...DEFAULT_PIXEL_PALETTE_PROPS.disabledIds];
  }
  const ids = new Set<string>();
  for (const item of value) {
    if (typeof item === "string" && item.length > 0) ids.add(item);
  }
  return [...ids];
}

export function getDisabledIds(): string[] {
  const props = Storer.load<PixelPaletteProps>(PIXEL_PALETTE_PROPS_KEY);
  return normalizeDisabledIds(props?.disabledIds);
}

export async function loadDisabledIds(): Promise<string[]> {
  const props = await Storer.loadPref<PixelPaletteProps>(
    PIXEL_PALETTE_PROPS_KEY,
  );
  return normalizeDisabledIds(props?.disabledIds);
}

/**
 * 串行化写入队列
 */
let writeQueue: Promise<unknown> = Promise.resolve();

/**
 * 启用/禁用一个方块并立即持久化。
 * @param id 调色板中的方块 ID（不带 `minecraft:` 前缀）
 * @param disabled true = 禁用，false = 启用
 * @returns 写入后的完整禁用集合
 */
export function setBlockDisabled(
  id: string,
  disabled: boolean,
): Promise<string[]> {
  const task = writeQueue.then(async () => {
    const ids = new Set(await loadDisabledIds());
    if (disabled) {
      ids.add(id);
    } else {
      ids.delete(id);
    }
    const disabledIds = [...ids];
    await Storer.savePartial<PixelPaletteProps>(PIXEL_PALETTE_PROPS_KEY, {
      disabledIds,
    });
    window.dispatchEvent(
      new CustomEvent(PIXEL_PALETTE_PROPS_CHANGED_EVENT, {
        detail: { disabledIds },
      }),
    );
    return disabledIds;
  });
  writeQueue = task.catch(() => undefined);
  return task;
}
