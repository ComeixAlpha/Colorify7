/** Minecraft 基岩版版本工具：目标版本过滤用。 */

/** 用于「使用最新版」的哨兵值。 */
export const TARGET_LATEST = "latest";

/** 当前调色板数据覆盖到的最新版本。 */
export const LATEST_MC_VERSION = "1.26.60";

/**
 * bedrock-samples 稳定标签对应的游戏版本（升序）。
 * 只用于给用户选择目标版本；版本比较本身是通用的。
 */
export const MC_VERSIONS: readonly string[] = [
  "1.19.30",
  "1.19.40",
  "1.19.50",
  "1.19.60",
  "1.19.70",
  "1.19.80",
  "1.20.0",
  "1.20.10",
  "1.20.30",
  "1.20.40",
  "1.20.50",
  "1.20.60",
  "1.20.70",
  "1.20.80",
  "1.21.0",
  "1.21.20",
  "1.21.30",
  "1.21.40",
  "1.21.50",
  "1.21.60",
  "1.21.70",
  "1.21.80",
  "1.21.90",
  "1.21.100",
  "1.21.110",
  "1.21.120",
  "1.21.130",
  "1.26.0",
  "1.26.10",
  "1.26.20",
  "1.26.30",
  "1.26.40",
  "1.26.50",
  "1.26.60",
];

function parts(v: string): number[] {
  const out = v
    .replace(/^v/, "")
    .split(".")
    .map((x) => Number.parseInt(x, 10))
    .map((x) => (Number.isFinite(x) ? x : 0));
  while (out.length < 4) out.push(0);
  return out;
}

/** 比较两个版本号（a < b 返回负数）。 */
export function compareVersion(a: string, b: string): number {
  const pa = parts(a);
  const pb = parts(b);
  for (let i = 0; i < 4; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

/** 方块在目标版本是否可用（since 为空视为一直可用）。 */
export function isAvailable(
  since: string | null | undefined,
  target: string,
): boolean {
  if (!since) return true;
  if (target === TARGET_LATEST) return true;
  return compareVersion(since, target) <= 0;
}
