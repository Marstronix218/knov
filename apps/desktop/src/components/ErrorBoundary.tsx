import { Component, ErrorInfo, ReactNode } from "react";

/**
 * Keeps a crash in one page from blanking the whole window. The sidebar stays
 * usable, and the parent keys this by route so navigating away resets it.
 */
export class ErrorBoundary extends Component<
  { children: ReactNode; onSendFeedback?: () => void },
  { error?: Error }
> {
  state: { error?: Error } = {};

  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    console.error("Knov view crashed", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="page">
        <section className="error-boundary" role="alert">
          <h2>This view hit an error</h2>
          <p><code>{error.message || "Unknown error"}</code></p>
          <div className="inline-actions">
            <button type="button" className="primary-button" onClick={() => this.setState({ error: undefined })}>Reload view</button>
            {this.props.onSendFeedback
              ? <button type="button" className="ghost-button" onClick={this.props.onSendFeedback}>Send feedback</button>
              : <span className="field-hint">Send feedback from the ⌘K menu.</span>}
          </div>
        </section>
      </div>
    );
  }
}
