export {};

declare global {
  interface Window {
    /** 由 index.html 的启动兜底脚本提供：展示启动/渲染错误诊断面板 */
    __colorifyShowError?: (error: unknown) => void;
    /** 由 index.html 的启动兜底脚本提供：标记应用已成功挂载 */
    __colorifyBooted?: () => void;
  }
}
