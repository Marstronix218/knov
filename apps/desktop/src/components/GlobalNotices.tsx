import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { AppNotice, dismissNotice, subscribeNotices } from "../lib/notices";

const AUTO_DISMISS_MS = 8000;

export function GlobalNotices() {
  const [notices, setNotices] = useState<AppNotice[]>([]);
  // One timer per notice, started when it first appears so later notices don't extend it.
  const timers = useRef(new Map<number, number>());
  useEffect(() => subscribeNotices(setNotices), []);
  useEffect(() => {
    const live = new Set(notices.map((notice) => notice.id));
    for (const [id, timer] of timers.current) {
      if (!live.has(id)) {
        window.clearTimeout(timer);
        timers.current.delete(id);
      }
    }
    for (const notice of notices) {
      if (!timers.current.has(notice.id)) {
        timers.current.set(notice.id, window.setTimeout(() => dismissNotice(notice.id), AUTO_DISMISS_MS));
      }
    }
  }, [notices]);
  useEffect(() => {
    const pending = timers.current;
    return () => {
      pending.forEach((timer) => window.clearTimeout(timer));
      pending.clear();
    };
  }, []);

  if (!notices.length) return null;
  return (
    <div className="global-notices" aria-live="assertive">
      {notices.map((notice) => (
        <p key={notice.id} className="notice error" role="alert">
          <span>{notice.text}</span>
          <button type="button" aria-label="Dismiss" onClick={() => dismissNotice(notice.id)}><X size={14} /></button>
        </p>
      ))}
    </div>
  );
}
