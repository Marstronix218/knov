import { Command, CornerDownLeft, Search } from "lucide-react";
import { KeyboardEvent as ReactKeyboardEvent, useEffect, useMemo, useRef, useState } from "react";

export interface PaletteCommand {
  id: string;
  label: string;
  group: string;
  hint?: string;
  keywords?: string;
  run: () => void | Promise<void>;
}

export function CommandPalette({ commands, onClose }: { commands: PaletteCommand[]; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const previous = useRef<HTMLElement | null>(
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  );

  const results = useMemo(() => {
    const terms = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
    return commands.filter((command) => {
      const haystack = `${command.label} ${command.group} ${command.keywords ?? ""}`.toLocaleLowerCase();
      return terms.every((term) => haystack.includes(term));
    });
  }, [commands, query]);

  useEffect(() => {
    input.current?.focus();
    const restore = previous.current;
    return () => restore?.focus();
  }, []);
  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    document.getElementById(`palette-${results[active]?.id}`)?.scrollIntoView?.({ block: "nearest" });
  }, [active, results]);

  const runCommand = (command?: PaletteCommand) => {
    if (!command) return;
    onClose();
    void command.run();
  };

  const onKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((index) => Math.min(index + 1, Math.max(results.length - 1, 0)));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((index) => Math.max(index - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      runCommand(results[active]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  };

  return (
    <div className="palette-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="palette" role="dialog" aria-modal="true" aria-label="Command menu" onMouseDown={(event) => event.stopPropagation()}>
        <label className="palette-input">
          <Search size={16} aria-hidden="true" />
          <input
            ref={input}
            role="combobox"
            aria-expanded="true"
            aria-controls="palette-results"
            aria-activedescendant={results[active] ? `palette-${results[active].id}` : undefined}
            aria-label="Search commands"
            placeholder="Go to a page or run a command…"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={onKeyDown}
          />
          <kbd>esc</kbd>
        </label>
        <ul id="palette-results" role="listbox" aria-label="Commands" className="palette-results">
          {results.length === 0 && <li className="palette-empty">No matching commands</li>}
          {results.map((command, index) => (
            <li
              key={command.id}
              id={`palette-${command.id}`}
              role="option"
              aria-selected={index === active}
              className={index === active ? "active" : ""}
              onMouseEnter={() => setActive(index)}
              onClick={() => runCommand(command)}
            >
              <span>{command.label}</span>
              <small>{command.group}</small>
              {command.hint && <kbd>{command.hint}</kbd>}
            </li>
          ))}
        </ul>
        <footer className="palette-footer">
          <span><Command size={12} aria-hidden="true" />K toggles this menu</span>
          <span><CornerDownLeft size={12} aria-hidden="true" /> to run</span>
        </footer>
      </section>
    </div>
  );
}
