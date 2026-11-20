import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
}

/**
 * React 渲染错误兜底：把错误交给 index.html 的启动诊断面板，
 * 避免整页变成没有信息的空白。
 */
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    window.__colorifyShowError?.(error);
    console.error("[colorify] render error:", error, info.componentStack);
  }

  render() {
    return this.state.hasError ? null : this.props.children;
  }
}
