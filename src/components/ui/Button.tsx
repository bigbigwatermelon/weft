import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

const button = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[var(--radius-md)] font-medium transition-[background-color,border-color,color,opacity] duration-150 ease-[var(--ease-out-quint)] disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-brand focus-visible:outline-offset-1 select-none",
  {
    variants: {
      variant: {
        primary:
          "bg-brand text-brand-ink hover:bg-brand/90 active:bg-brand-press",
        default:
          "bg-raised text-ink border border-border hover:border-border-strong hover:bg-[oklch(0.29_0.014_292)]",
        ghost: "text-ink-muted hover:text-ink hover:bg-brand-ghost",
        danger:
          "text-danger border border-transparent hover:bg-[oklch(0.64_0.2_25/0.12)]",
      },
      size: {
        sm: "h-7 px-2.5 text-[12px]",
        md: "h-8 px-3 text-[13px]",
        icon: "h-7 w-7 p-0",
      },
    },
    defaultVariants: { variant: "default", size: "md" },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof button> {}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => (
    <button
      ref={ref}
      className={cn(button({ variant, size }), className)}
      {...props}
    />
  ),
);
Button.displayName = "Button";
