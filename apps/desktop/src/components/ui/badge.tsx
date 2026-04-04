import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium ring-1 ring-inset",
  {
    variants: {
      variant: {
        default:
          "bg-secondary/60 text-secondary-foreground ring-border",
        live: "bg-live/15 text-live ring-live/25",
        accent:
          "bg-primary/10 text-primary ring-primary/20",
        outline:
          "text-muted-foreground ring-border",
        success:
          "bg-emerald-500/10 text-emerald-600 ring-emerald-500/20 dark:text-emerald-400",
        destructive:
          "bg-destructive/10 text-destructive ring-destructive/20",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
