import { useEffect, useReducer } from "react";

const sessionStore = new Map<string, unknown>();

/**
 * 页面会话状态模板。
 *
 * 每个页面定义一个继承它的子类，声明自己的参数结构（interface），
 * 实例在整个进程内全局唯一：
 *
 * - React 侧：组件内调用 `state.use()` 绑定，任何 `update()` 自动触发重渲染
 * - 后端侧：通过 `snapshot` / `toJSON()` 直接读取并传给 Tauri 命令
 * - 生命周期：切换页面不丢失；关闭 APP / 刷新后重置为初始值
 *
 * @example
 * ```ts
 * export interface ParticleParams { resizeX: number | null; ... }
 *
 * export class ParticlePageState extends PageState<ParticleParams> {
 *   constructor() {
 *     super({ resizeX: null, resizeY: null, height: null, resizeInterpolation: "Nearest" });
 *   }
 * }
 *
 * export const particlePageState = new ParticlePageState();
 * ```
 */
export abstract class PageState<T extends object> {
  private readonly key: string;
  private data: T;
  private listeners = new Set<() => void>();

  constructor(initial: T, key?: string) {
    this.key = key ?? new.target.name;
    const saved = sessionStore.get(this.key) as Partial<T> | undefined;
    this.data = { ...initial, ...saved };
  }

  get snapshot(): Readonly<T> {
    return this.data;
  }

  toJSON(): string {
    return JSON.stringify(this.data);
  }

  protected update(patch: Partial<T>): void {
    this.data = { ...this.data, ...patch };
    sessionStore.set(this.key, this.data);
    for (const listener of this.listeners) listener();
  }

  use() {
    const [, forceRender] = useReducer((x: number) => x + 1, 0);

    useEffect(() => {
      this.listeners.add(forceRender);
      return () => {
        this.listeners.delete(forceRender);
      };
    }, []);

    return {
      data: this.data as T,
      update: (patch: Partial<T>) => this.update(patch),
    };
  }
}
