import clsx from "clsx";

export type TabId = "home" | "launch_options" | "settings" | "about";

interface Props {
  active: TabId;
  onChange: (tab: TabId) => void;
}

const TABS: { id: TabId; label: string; icon: string }[] = [
  { id: "home", label: "首页", icon: "▶" },
  { id: "launch_options", label: "启动项", icon: "⚙" },
  { id: "settings", label: "设置", icon: "✦" },
  { id: "about", label: "关于", icon: "ⓘ" },
];

export function Sidebar({ active, onChange }: Props) {
  return (
    <aside className="w-[88px] shrink-0 flex flex-col items-center py-5 gap-1 border-r border-white/5">
      <div className="size-12 rounded-xl glass flex items-center justify-center text-lg font-bold mb-4">
        R5R
      </div>
      {TABS.map((t) => (
        <button
          key={t.id}
          onClick={() => onChange(t.id)}
          className={clsx(
            "w-16 h-16 rounded-xl flex flex-col items-center justify-center gap-1 transition-all",
            "hover:bg-white/5",
            active === t.id
              ? "bg-white/8 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]"
              : "text-white/55",
          )}
        >
          <span className="text-lg leading-none">{t.icon}</span>
          <span className="text-[11px] leading-none">{t.label}</span>
        </button>
      ))}
      <div className="flex-1" />
      <div className="text-[10px] text-white/30">v0.5.0</div>
    </aside>
  );
}
