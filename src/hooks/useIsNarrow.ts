import { useEffect, useState } from "react";

const NARROW_QUERY = "(max-width: 959.98px)";

export function useIsNarrow(): boolean {
  const [isNarrow, setIsNarrow] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia(NARROW_QUERY).matches,
  );

  useEffect(() => {
    const mql = window.matchMedia(NARROW_QUERY);
    const handler = (e: MediaQueryListEvent) => setIsNarrow(e.matches);
    mql.addEventListener("change", handler);
    // 同步一次，处理 SSR / 初始渲染后的变化
    setIsNarrow(mql.matches);
    return () => mql.removeEventListener("change", handler);
  }, []);

  return isNarrow;
}
