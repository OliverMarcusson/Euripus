import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-primary px-4 py-2 text-primary-foreground shadow-sm hover:bg-primary/90 active:scale-[0.98]",
        secondary:
          "bg-secondary px-4 py-2 text-secondary-foreground hover:bg-secondary/80 active:scale-[0.98]",
        ghost:
          "px-3 py-2 text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        outline:
          "border border-border bg-background px-4 py-2 text-foreground shadow-sm hover:bg-accent hover:text-accent-foreground active:scale-[0.98]",
        destructive:
          "bg-destructive px-4 py-2 text-destructive-foreground shadow-sm hover:bg-destructive/90 active:scale-[0.98]",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9",
        sm: "h-8 px-3 text-xs",
        lg: "h-10 px-5",
        icon: "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => (
    <button ref={ref} className={cn(buttonVariants({ variant, size }), className)} {...props} />
  ),
);

Button.displayName = "Button";
