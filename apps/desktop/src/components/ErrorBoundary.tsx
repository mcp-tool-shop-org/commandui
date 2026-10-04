import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

type Props = { children: ReactNode };
type State = {
  hasError: boolean;
  error: Error | null;
  componentStack: string | null;
  showDetails: boolean;
};

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, componentStack: null, showDetails: false };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({ componentStack: errorInfo.componentStack ?? null });
    const payload = {
      type: "render_error",
      message: error.message,
      stack: error.stack ?? null,
      componentStack: errorInfo.componentStack ?? null,
      timestamp: new Date().toISOString(),
      userAgent: typeof navigator !== "undefined" ? navigator.userAgent : null,
    };
    console.error("[CommandUI] Uncaught render error:", payload);
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

  private toggleDetails = () => {
    this.setState((prev) => ({ showDetails: !prev.showDetails }));
  };

  render() {
    if (this.state.hasError) {
      const { error, componentStack, showDetails } = this.state;
      const details = [
        error?.stack ?? "",
        componentStack ?? "",
      ].filter(Boolean).join("\n\n");
      return (
        <div className="error-boundary-fallback">
          <h2 className="error-boundary-heading">
            CommandUI encountered an unexpected error
          </h2>
          <div className="error-boundary-message">
            {error?.message ?? "Unknown error"}
          </div>
          {details && (
            <div style={{ marginBottom: 12 }}>
              <button type="button" className="link-btn" onClick={this.toggleDetails}>
                {showDetails ? "Hide details" : "Show details"}
              </button>
            </div>
          )}
          {showDetails && details && (
            <pre
              className="error-boundary-message"
              style={{ textAlign: "left", maxHeight: 320, overflow: "auto" }}
            >
              {details}
            </pre>
          )}
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
