import * as RD from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export const Dialog = RD.Root;
export const DialogTrigger = RD.Trigger;

export function DialogContent({
  title,
  description,
  children,
  className,
}: {
  title: string;
  description?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <RD.Portal>
      <RD.Overlay className="atlas-overlay fixed inset-0 z-50 bg-black/55 backdrop-blur-[1px]" />
      <RD.Content
        className={cn(
          "atlas-pop fixed left-1/2 top-1/2 z-50 w-[min(440px,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2",
          "rounded-[var(--radius-lg)] border border-border bg-surface p-5 shadow-[0_8px_28px_-8px_rgba(0,0,0,0.6)]",
          className,
        )}
      >
        <div className="mb-4 flex items-start justify-between gap-4">
          <div className="flex flex-col gap-1">
            <RD.Title className="text-[15px] font-semibold tracking-tight text-ink">
              {title}
            </RD.Title>
            {description && (
              <RD.Description className="text-[12px] text-ink-faint">
                {description}
              </RD.Description>
            )}
          </div>
          <RD.Close
            className="-mr-1 -mt-1 grid h-7 w-7 place-items-center rounded-[var(--radius-md)] text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
            aria-label="Close"
          >
            <X size={15} />
          </RD.Close>
        </div>
        {children}
      </RD.Content>
    </RD.Portal>
  );
}

export const DialogClose = RD.Close;
