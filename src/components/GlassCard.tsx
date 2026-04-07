import { ReactNode } from "react";
import clsx from "clsx";

interface Props {
  children: ReactNode;
  className?: string;
  padding?: boolean;
}

export function GlassCard({ children, className, padding = true }: Props) {
  return (
    <div className={clsx("glass", padding && "p-5", className)}>{children}</div>
  );
}

interface SectionHeaderProps {
  icon?: ReactNode;
  title: string;
  subtitle?: string;
  right?: ReactNode;
}

export function SectionHeader({ icon, title, subtitle, right }: SectionHeaderProps) {
  return (
    <div className="flex items-center gap-3 mb-3">
      {icon && (
        <div className="size-9 rounded-lg bg-white/5 flex items-center justify-center text-base">
          {icon}
        </div>
      )}
      <div className="flex-1 min-w-0">
        <div className="text-[15px] font-medium">{title}</div>
        {subtitle && (
          <div className="text-xs text-white/50 mt-0.5">{subtitle}</div>
        )}
      </div>
      {right}
    </div>
  );
}
