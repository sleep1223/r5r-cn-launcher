import { ButtonHTMLAttributes, ReactNode } from "react";
import clsx from "clsx";

type Variant = "primary" | "secondary" | "danger" | "warn" | "success";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  children: ReactNode;
  size?: "md" | "lg";
}

const VARIANTS: Record<Variant, string> = {
  primary:
    "bg-gradient-to-b from-[#5b8def] to-[#3a6ed1] text-white shadow-lg shadow-blue-500/20 hover:brightness-110",
  secondary: "bg-white/8 text-white hover:bg-white/12",
  danger:
    "bg-gradient-to-b from-[#ef4444] to-[#b91c1c] text-white shadow-lg shadow-red-500/20 hover:brightness-110",
  warn:
    "bg-gradient-to-b from-[#f59e0b] to-[#b45309] text-white shadow-lg shadow-amber-500/20 hover:brightness-110",
  success:
    "bg-gradient-to-b from-[#10b981] to-[#047857] text-white shadow-lg shadow-emerald-500/20 hover:brightness-110",
};

export function PrimaryButton({
  variant = "primary",
  size = "md",
  className,
  children,
  ...rest
}: Props) {
  return (
    <button
      {...rest}
      className={clsx(
        "rounded-xl font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed",
        size === "lg" ? "px-6 py-3 text-base" : "px-4 py-2 text-sm",
        VARIANTS[variant],
        className,
      )}
    >
      {children}
    </button>
  );
}
