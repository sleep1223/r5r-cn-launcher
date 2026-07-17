import clsx from "clsx";
import type { InputDevice } from "../api";

interface Props {
  device?: InputDevice | string | null;
  compact?: boolean;
}

function deviceMeta(device?: InputDevice | string | null) {
  const normalized = device?.trim().toLowerCase();
  if (normalized === "controller" || normalized === "gamepad") {
    return {
      icon: "🎮",
      label: "手柄",
      className: "bg-purple-500/10 text-purple-200 border-purple-400/20",
    };
  }
  if (
    normalized === "keyboard_mouse" ||
    normalized === "mouse_keyboard" ||
    normalized === "keyboard" ||
    normalized === "mouse"
  ) {
    return {
      icon: "⌨",
      label: "键鼠",
      className: "bg-blue-500/10 text-blue-200 border-blue-400/20",
    };
  }
  return {
    icon: "?",
    label: "未知",
    className: "bg-white/5 text-white/45 border-white/10",
  };
}

export function InputDeviceBadge({ device, compact = false }: Props) {
  const meta = deviceMeta(device);
  return (
    <span
      title={`输入设备：${meta.label}`}
      className={clsx(
        "inline-flex items-center rounded-md border whitespace-nowrap",
        compact ? "gap-1 px-1.5 py-0.5 text-[10px]" : "gap-1.5 px-2 py-1 text-[11px]",
        meta.className,
      )}
    >
      <span aria-hidden="true" className="leading-none">
        {meta.icon}
      </span>
      {meta.label}
    </span>
  );
}
