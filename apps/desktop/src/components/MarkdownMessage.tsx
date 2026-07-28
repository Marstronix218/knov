import { createElement, type ReactNode } from "react";

const unorderedItem = /^\s*[-*+]\s+(.+)$/;
const orderedItem = /^\s*\d+[.)]\s+(.+)$/;
const heading = /^(#{1,6})\s+(.+)$/;
const fence = /^```([\w-]*)\s*$/;
const blockquote = /^\s*>\s?(.*)$/;
const horizontalRule = /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/;
const inlineToken =
  /(`[^`\n]+`|\*\*[^*\n]+\*\*|__[^_\n]+__|\[[^\]\n]+\]\((?:https?:\/\/|mailto:)[^) \n]+\)|\*[^*\n]+\*|_[^_\n]+_)/g;

function renderInline(text: string, keyPrefix: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let cursor = 0;

  for (const match of text.matchAll(inlineToken)) {
    const index = match.index ?? 0;
    if (index > cursor) {
      nodes.push(text.slice(cursor, index));
    }

    const token = match[0];
    const key = `${keyPrefix}-${index}`;

    if (token.startsWith("`")) {
      nodes.push(<code key={key}>{token.slice(1, -1)}</code>);
    } else if (token.startsWith("**") || token.startsWith("__")) {
      nodes.push(<strong key={key}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("[")) {
      const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      nodes.push(
        link ? (
          <a href={link[2]} key={key} rel="noreferrer" target="_blank">
            {link[1]}
          </a>
        ) : (
          token
        ),
      );
    } else {
      nodes.push(<em key={key}>{token.slice(1, -1)}</em>);
    }

    cursor = index + token.length;
  }

  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }

  return nodes;
}

function startsBlock(line: string): boolean {
  return (
    !line.trim() ||
    heading.test(line) ||
    fence.test(line) ||
    unorderedItem.test(line) ||
    orderedItem.test(line) ||
    blockquote.test(line) ||
    horizontalRule.test(line)
  );
}

export function MarkdownMessage({ children }: { children: string }) {
  const lines = children.replace(/\r\n?/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];

    if (!line.trim()) {
      index += 1;
      continue;
    }

    const fenceMatch = line.match(fence);
    if (fenceMatch) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !fence.test(lines[index])) {
        code.push(lines[index]);
        index += 1;
      }
      if (index < lines.length) index += 1;
      blocks.push(
        <pre key={`code-${index}`}>
          <code className={fenceMatch[1] ? `language-${fenceMatch[1]}` : undefined}>
            {code.join("\n")}
          </code>
        </pre>,
      );
      continue;
    }

    const headingMatch = line.match(heading);
    if (headingMatch) {
      const level = headingMatch[1].length;
      blocks.push(
        createElement(
          `h${level}`,
          { key: `heading-${index}` },
          renderInline(headingMatch[2], `heading-${index}`),
        ),
      );
      index += 1;
      continue;
    }

    const listMatch = line.match(unorderedItem) ?? line.match(orderedItem);
    if (listMatch) {
      const ordered = orderedItem.test(line);
      const matcher = ordered ? orderedItem : unorderedItem;
      const items: ReactNode[] = [];

      while (index < lines.length) {
        const item = lines[index].match(matcher);
        if (!item) break;
        items.push(<li key={`item-${index}`}>{renderInline(item[1], `item-${index}`)}</li>);
        index += 1;
      }

      blocks.push(
        ordered ? <ol key={`list-${index}`}>{items}</ol> : <ul key={`list-${index}`}>{items}</ul>,
      );
      continue;
    }

    const quoteMatch = line.match(blockquote);
    if (quoteMatch) {
      const quote: string[] = [];
      while (index < lines.length) {
        const part = lines[index].match(blockquote);
        if (!part) break;
        quote.push(part[1]);
        index += 1;
      }
      blocks.push(
        <blockquote key={`quote-${index}`}>
          <p>{renderInline(quote.join(" "), `quote-${index}`)}</p>
        </blockquote>,
      );
      continue;
    }

    if (horizontalRule.test(line)) {
      blocks.push(<hr key={`rule-${index}`} />);
      index += 1;
      continue;
    }

    const paragraph: string[] = [line.trim()];
    index += 1;
    while (index < lines.length && !startsBlock(lines[index])) {
      paragraph.push(lines[index].trim());
      index += 1;
    }
    blocks.push(
      <p key={`paragraph-${index}`}>
        {renderInline(paragraph.join(" "), `paragraph-${index}`)}
      </p>,
    );
  }

  return <div className="message-content markdown-content">{blocks}</div>;
}
