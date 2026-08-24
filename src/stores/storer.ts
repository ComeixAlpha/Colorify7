
export class Storer {
  private static cache: Map<string, any> = new Map();

  private static defaults: Map<string, any> = new Map();

  private static isTauri(): boolean {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  public static registerDefaults(key: string, defaultValue: any) {
    this.defaults.set(key, defaultValue);
  }

  public static load<T>(key: string): T {
    if (this.cache.has(key)) return this.cache.get(key);
    const defaultValue = this.defaults.get(key);
    return defaultValue;
  }

  public static async loadPref<T>(key: string): Promise<T> {
    if (this.cache.has(key)) return this.cache.get(key);

    if (!this.defaults.has(key)) {
      throw new Error(`No default value registered for key: ${key}`);
    }

    const defaultValue = this.defaults.get(key) as T;

    try {
      let loaded: T;
      if (this.isTauri()) {
        const { load } = await import("@tauri-apps/plugin-store");
        const store = await load(`pref.json`, { autoSave: true });
        const saved = await store.get<Partial<T>>(key);
        loaded = { ...defaultValue, ...saved };
      } else {
        const raw = window.localStorage.getItem(`colorify.pref.${key}`);
        loaded = raw
          ? { ...defaultValue, ...(JSON.parse(raw) as Partial<T>) }
          : defaultValue;
      }
      this.cache.set(key, loaded);
      return loaded;
    } catch {
      this.cache.set(key, defaultValue);
      return defaultValue;
    }
  }

  private static async savePref<T>(key: string, value: T): Promise<void> {
    try {
      this.cache.set(key, value);
      if (this.isTauri()) {
        const { load } = await import("@tauri-apps/plugin-store");
        const store = await load(`pref.json`, { autoSave: true });
        await store.set(key, value);
        await store.save();
        return;
      }
      window.localStorage.setItem(`colorify.pref.${key}`, JSON.stringify(value));
    } catch {
    }
  }

  public static async savePartial<T extends Partial<T>>(
    key: string,
    params: T,
  ): Promise<void> {
    const current = await this.loadPref<T>(key);
    await this.savePref(key, { ...current, ...params });
  }
}


