import { cn } from "../../lib/cn";

export function Toggle({
  on,
  onChange,
  label,
}: {
  on: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={cn(
        "relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-0 transition-colors duration-150",
        on ? "bg-brand" : "bg-border-strong",
      )}
    >
      <span
        className={cn(
          "absolute left-0.5 top-0.5 inline-block h-[18px] w-[18px] rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.3)] transition-transform duration-150",
          on ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}
