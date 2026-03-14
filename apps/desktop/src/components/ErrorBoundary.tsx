import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

type Props = { children: ReactNode };
type State = {
  hasError: boolean;
  error: Error | null;
  componentStack: string | null;
};

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, componentStack: null };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({ componentStack: errorInfo.componentStack ?? null });
    console.error("[CommandUI] Uncaught render error:", error, errorInfo);
  }

  private handleCopy = () => {
    const { error, componentStack } = this.state;
    const text = [
      error?.message ?? "Unknown error",
      "",
      error?.stack ?? "",
      "",
      componentStack ?? "",
    ].join("\n");
    void navigator.clipboard.writeText(text);
  };

  private handleReload = () => {
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary-fallback">
          <h2 className="error-boundary-heading">
            CommandUI encountered an unexpected error
          </h2>
          <div className="error-boundary-message">
            {this.state.error?.message ?? "Unknown error"}
          </div>
          <div className="error-boundary-actions">
            <button type="button" onClick={this.handleCopy}>
              Copy Error Details
            </button>
            <button type="button" onClick={this.handleReload}>
              Reload App
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
