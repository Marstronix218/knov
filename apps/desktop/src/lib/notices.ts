/**
 * App-wide, non-blocking error notices for failures that have no inline place
 * to show up: unhandled promise rejections, uncaught errors, and actions whose
 * UI already closed.
 */
export interface AppNotice {
  id: number;
  text: string;
}

type Listener = (notices: AppNotice[]) => void;

const MAX_NOTICES = 3;
let notices: AppNotice[] = [];
let nextId = 0;
const listeners = new Set<Listener>();

function publish(next: AppNotice[]) {
  notices = next;
  listeners.forEach((listener) => listener(notices));
}

export function notifyError(text: string): void {
  if (notices.some((notice) => notice.text === text)) return;
  publish([...notices, { id: ++nextId, text }].slice(-MAX_NOTICES));
}

export function dismissNotice(id: number): void {
  publish(notices.filter((notice) => notice.id !== id));
}

export function subscribeNotices(listener: Listener): () => void {
  listeners.add(listener);
  listener(notices);
  return () => listeners.delete(listener);
}
