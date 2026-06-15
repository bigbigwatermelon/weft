import { memo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { api } from "../lib/api";

/**
 * Renders agent output as markdown — headings, lists, code, tables, links —
 * scoped + sized to fit the transcript (no global prose plugin needed). Links
 * open in the OS browser via the opener, never inside the app webview.
 */
export const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <div className="atlas-md text-[12.5px] leading-relaxed text-ink">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              onClick={(e) => {
                e.preventDefault();
                if (href) void api.openUrl(href);
              }}
              className="text-brand underline decoration-brand/40 underline-offset-2 hover:decoration-brand"
            >
              {children}
            </a>
          ),
          code: ({ className, children }) => {
            const inline = !String(className ?? "").includes("language-");
            return inline ? (
              <code className="rounded bg-raised px-1 py-0.5 font-mono text-[11.5px] text-ink">
                {children}
              </code>
            ) : (
              <code className="font-mono text-[11.5px]">{children}</code>
            );
          },
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
});
