import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import clsx from "clsx";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { useSettings } from "../hooks/useSettings";
import {
  composeLaunchArgs,
  getLaunchOptionCatalog,
  validateLaunchArgs,
} from "../ipc/launchOptions";
import {
  LaunchOptionCatalog,
  LaunchOptionSelection,
  LaunchWarning,
  OptionEntry,
  OptionValue,
  SelectionEntry,
} from "../ipc/types";

interface ResolutionPreset {
  width: number;
  height: number;
  label: string;
}

// Resolution presets. Standard tier names (720P / 1K / 2K / 4K) stay as-is;
// non-standard variants in the same tier get an aspect-ratio label so the
// user can tell at a glance which one they need. Sorted by total pixel count.
const RESOLUTION_PRESETS: ResolutionPreset[] = [
  { width: 1024, height: 768,  label: "1024×768 (4:3)" },
  { width: 1280, height: 720,  label: "1280×720 (720P)" },
  { width: 1280, height: 960,  label: "1280×960 (4:3)" },
  { width: 1600, height: 900,  label: "1600×900 (16:9)" },
  { width: 1680, height: 1050, label: "1680×1050 (16:10)" },
  { width: 1920, height: 1080, label: "1920×1080 (1K)" },
  { width: 1920, height: 1200, label: "1920×1200 (16:10)" },
  { width: 2560, height: 1440, label: "2560×1440 (2K)" },
  { width: 3840, height: 2160, label: "3840×2160 (4K)" },
];

const FOV_PRESETS: number[] = [70, 90, 100, 110, 120];

interface DetailPopover {
  entry: OptionEntry;
  x: number;
  y: number;
}

export function LaunchOptionsTab() {
  const { settings, update } = useSettings();
  const [catalog, setCatalog] = useState<LaunchOptionCatalog | null>(null);
  const [selection, setSelection] = useState<LaunchOptionSelection>({ items: {} });
  const [composed, setComposed] = useState<string[]>([]);
  const [warnings, setWarnings] = useState<LaunchWarning[]>([]);
  const [copyFlash, setCopyFlash] = useState(false);
  const [detail, setDetail] = useState<DetailPopover | null>(null);
  const saveTimer = useRef<number | null>(null);
  const initialLoadDone = useRef(false);

  // Dismiss the right-click popover on any document click or Escape.
  useEffect(() => {
    if (!detail) return;
    const close = () => setDetail(null);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setDetail(null);
    };
    window.addEventListener("click", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [detail]);

  // Load the catalog once.
  useEffect(() => {
    (async () => {
      try {
        const c = await getLaunchOptionCatalog();
        setCatalog(c);
      } catch (e) {
        console.error("get_launch_option_catalog failed", e);
      }
    })();
  }, []);

  // Hydrate selection from settings on first arrival.
  useEffect(() => {
    if (!settings || initialLoadDone.current) return;
    const persisted = settings.launch_option_selection as
      | LaunchOptionSelection
      | null
      | undefined;
    if (persisted && persisted.items) {
      setSelection(persisted);
    }
    initialLoadDone.current = true;
  }, [settings]);

  // Recompute composed args + warnings whenever selection changes.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [args, w] = await Promise.all([
          composeLaunchArgs(selection),
          validateLaunchArgs(selection),
        ]);
        if (!cancelled) {
          setComposed(args);
          setWarnings(w);
        }
      } catch (e) {
        console.error(e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selection]);

  // Debounced auto-save (300ms after last edit).
  useEffect(() => {
    if (!initialLoadDone.current) return;
    if (saveTimer.current) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      void update({ launch_option_selection: selection });
    }, 300);
    return () => {
      if (saveTimer.current) window.clearTimeout(saveTimer.current);
    };
  }, [selection, update]);

  const grouped = useMemo(() => {
    if (!catalog) return [];
    return catalog.categories.map((cat) => ({
      cat,
      entries: catalog.entries.filter((e) => e.category === cat.id),
    }));
  }, [catalog]);

  const isEnabled = (id: string): boolean => {
    if (id in selection.items) return selection.items[id].enabled;
    return catalog?.entries.find((e) => e.id === id)?.default_enabled ?? false;
  };

  const getValue = (id: string): OptionValue | null => {
    const item = selection.items[id];
    if (item?.value) return item.value;
    return catalog?.entries.find((e) => e.id === id)?.default_value ?? null;
  };

  const setEntry = (id: string, patch: Partial<SelectionEntry>) => {
    setSelection((prev) => {
      const existing: SelectionEntry =
        prev.items[id] ?? {
          enabled: catalog?.entries.find((e) => e.id === id)?.default_enabled ?? false,
          value:
            catalog?.entries.find((e) => e.id === id)?.default_value ?? null,
        };
      return {
        items: { ...prev.items, [id]: { ...existing, ...patch } },
      };
    });
  };

  const composedString = composed.join(" ");

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(composedString);
      setCopyFlash(true);
      window.setTimeout(() => setCopyFlash(false), 1500);
    } catch (e) {
      console.error(e);
    }
  };

  const handleResetDefaults = () => {
    setSelection({ items: {} });
  };

  if (!catalog) {
    return <div className="p-8 text-white/60">加载启动项目录中…</div>;
  }

  return (
    <div className="grid grid-cols-[1fr_360px] gap-5 p-6 h-full">
      {/* LEFT: categorized list */}
      <div className="overflow-y-auto pr-2 space-y-5">
        {grouped.map(({ cat, entries }) => (
          <GlassCard key={cat.id}>
            <SectionHeader title={cat.label_zh} />
            <div className="space-y-2">
              {entries.map((entry) => (
                <EntryRow
                  key={entry.id}
                  entry={entry}
                  enabled={isEnabled(entry.id)}
                  value={getValue(entry.id)}
                  onToggle={(en) => setEntry(entry.id, { enabled: en })}
                  onValue={(v) => setEntry(entry.id, { value: v })}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setDetail({ entry, x: e.clientX, y: e.clientY });
                  }}
                />
              ))}
            </div>
          </GlassCard>
        ))}
      </div>

      {/* RIGHT: live preview */}
      <div className="space-y-4">
        <GlassCard>
          <SectionHeader title="启动参数预览" subtitle="编辑后自动保存。" />
          <div className="font-mono text-xs bg-black/40 rounded-lg p-3 max-h-40 overflow-y-auto break-all min-h-[64px] text-emerald-300">
            {composedString || <span className="text-white/30">（无）</span>}
          </div>
          <div className="flex gap-2 mt-3">
            <PrimaryButton variant="primary" onClick={handleCopy}>
              {copyFlash ? "已复制 ✓" : "一键复制"}
            </PrimaryButton>
            <PrimaryButton variant="secondary" onClick={handleResetDefaults}>
              恢复默认
            </PrimaryButton>
          </div>
        </GlassCard>

        {warnings.length > 0 && (
          <GlassCard>
            <SectionHeader title={`警告 (${warnings.length})`} />
            <ul className="space-y-2">
              {warnings.map((w, i) => (
                <li
                  key={i}
                  className={clsx(
                    "text-xs px-3 py-2 rounded-lg",
                    w.severity === "danger"
                      ? "bg-red-500/10 text-red-300"
                      : "bg-amber-500/10 text-amber-300",
                  )}
                >
                  ⚠ {w.message_zh}
                </li>
              ))}
            </ul>
          </GlassCard>
        )}

        <GlassCard>
          <div className="text-xs text-white/50 leading-relaxed">
            提示：启动项配置会自动保存到本地，首页【启动游戏】会读取这里的设置传给 r5apex.exe。右键任一项可查看详细描述。
          </div>
        </GlassCard>
      </div>

      {detail && <DetailPopover detail={detail} />}
    </div>
  );
}

function DetailPopover({ detail }: { detail: DetailPopover }) {
  // Render via portal so the popover sits above any overflow:hidden parents
  // and gets positioned in viewport coordinates.
  const maxW = 360;
  const left = Math.min(detail.x, window.innerWidth - maxW - 16);
  const top = Math.min(detail.y, window.innerHeight - 200);

  const flagPreview = previewFlags(detail.entry);

  return createPortal(
    <div
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
      style={{ position: "fixed", left, top, maxWidth: maxW, zIndex: 9999 }}
      className="rounded-xl border border-white/15 bg-[#181c22]/95 backdrop-blur-md shadow-2xl p-4 text-sm"
    >
      <div className="flex items-center gap-2 mb-2">
        <span className="font-semibold">{detail.entry.label_zh}</span>
        {detail.entry.risk !== "none" && (
          <span
            className={clsx(
              "text-[10px] px-1.5 py-0.5 rounded",
              detail.entry.risk === "danger"
                ? "bg-red-500/15 text-red-300"
                : "bg-amber-500/15 text-amber-300",
            )}
          >
            {detail.entry.risk === "danger" ? "高风险" : "需注意"}
          </span>
        )}
      </div>
      <div className="text-xs text-white/70 leading-relaxed whitespace-pre-wrap">
        {detail.entry.description_zh}
      </div>
      {flagPreview && (
        <div className="mt-3 font-mono text-[11px] bg-black/40 rounded px-2 py-1.5 text-emerald-300 break-all">
          {flagPreview}
        </div>
      )}
      <div className="text-[10px] text-white/30 mt-2">id: {detail.entry.id}</div>
    </div>,
    document.body,
  );
}

/** Best-effort one-liner showing the args this entry would emit. */
function previewFlags(entry: OptionEntry): string | null {
  switch (entry.kind.type) {
    case "toggle":
      return entry.kind.args.join(" ");
    case "int":
      return `${entry.kind.flag} <${entry.kind.min}-${entry.kind.max}>`;
    case "float":
      return `${entry.kind.flag} <${entry.kind.min}-${entry.kind.max}>`;
    case "int_pair":
      return `${entry.kind.x_flag} <W> ${entry.kind.y_flag} <H>`;
    case "enum":
      return `${entry.kind.flag} <${entry.kind.choices.map(([v]) => v).join("|")}>`;
    case "enum_args":
      return entry.kind.choices
        .map((c) => `${c.label_zh}: ${c.args.join(" ")}`)
        .join("\n");
    case "fov_degrees":
      return `${entry.kind.flag} <${entry.kind.min}°-${entry.kind.max}°/${entry.kind.base}>`;
    case "string":
      return `${entry.kind.flag} <${entry.kind.placeholder}>`;
  }
}

interface EntryRowProps {
  entry: OptionEntry;
  enabled: boolean;
  value: OptionValue | null;
  onToggle: (en: boolean) => void;
  onValue: (v: OptionValue) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}

function EntryRow({
  entry,
  enabled,
  value,
  onToggle,
  onValue,
  onContextMenu,
}: EntryRowProps) {
  const isCombo =
    entry.kind.type === "toggle" && entry.kind.is_combo === true;
  return (
    <div
      onContextMenu={onContextMenu}
      className={clsx(
        "rounded-lg border p-3 transition-all select-none",
        enabled
          ? "border-blue-400/30 bg-blue-400/5"
          : "border-white/5 hover:bg-white/[0.03]",
      )}
    >
      <label className="flex items-start gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => onToggle(e.target.checked)}
          className="mt-1 size-4 accent-blue-400"
        />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium flex items-center gap-2">
            {entry.label_zh}
            {isCombo && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-purple-500/15 text-purple-300">
                组合
              </span>
            )}
            {entry.risk === "caution" && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300">
                需注意
              </span>
            )}
          </div>
          <div className="text-[11px] text-white/50 mt-0.5 leading-relaxed line-clamp-2">
            {entry.description_zh}
          </div>
        </div>
      </label>
      {enabled && <div className="mt-3 ml-7">{renderInput(entry, value, onValue)}</div>}
    </div>
  );
}

function renderInput(
  entry: OptionEntry,
  value: OptionValue | null,
  onValue: (v: OptionValue) => void,
) {
  switch (entry.kind.type) {
    case "toggle":
      return null;
    case "int": {
      const v = value?.type === "int" ? value.value : entry.kind.min;
      return (
        <input
          type="number"
          min={entry.kind.min}
          max={entry.kind.max}
          value={v}
          onChange={(e) =>
            onValue({ type: "int", value: Number(e.target.value) })
          }
          className="!w-32"
        />
      );
    }
    case "float": {
      const v = value?.type === "float" ? value.value : entry.kind.min;
      return (
        <input
          type="number"
          min={entry.kind.min}
          max={entry.kind.max}
          step={entry.kind.step}
          value={v}
          onChange={(e) =>
            onValue({ type: "float", value: Number(e.target.value) })
          }
          className="!w-32"
        />
      );
    }
    case "fov_degrees": {
      const v = value?.type === "int" ? value.value : entry.kind.max;
      // Degrees-stored FOV: presets + slider + number input. Backend converts
      // degrees → +cl_fovScale at compose time (degrees / base).
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap gap-1.5">
            {FOV_PRESETS.map((deg) => (
              <button
                key={deg}
                type="button"
                onClick={() => onValue({ type: "int", value: deg })}
                className={clsx(
                  "text-[11px] px-2 py-1 rounded border transition-all",
                  v === deg
                    ? "border-blue-400/60 bg-blue-400/15 text-white"
                    : "border-white/10 text-white/60 hover:bg-white/5",
                )}
              >
                {deg}°
              </button>
            ))}
          </div>
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={entry.kind.min}
              max={entry.kind.max}
              step={1}
              value={v}
              onChange={(e) =>
                onValue({ type: "int", value: Number(e.target.value) })
              }
              className="flex-1"
            />
            <input
              type="number"
              min={entry.kind.min}
              max={entry.kind.max}
              step={1}
              value={v}
              onChange={(e) =>
                onValue({ type: "int", value: Number(e.target.value) })
              }
              className="!w-20 tabular-nums"
            />
            <span className="text-[11px] text-white/40">度</span>
          </div>
        </div>
      );
    }
    case "int_pair": {
      const [w, h] =
        value?.type === "int_pair" ? value.value : [1920, 1080];
      const matchedPreset = RESOLUTION_PRESETS.find(
        (p) => p.width === w && p.height === h,
      );
      return (
        <div className="space-y-2">
          <div className="flex flex-wrap gap-1.5">
            {RESOLUTION_PRESETS.map((p) => (
              <button
                key={`${p.width}x${p.height}`}
                type="button"
                onClick={() =>
                  onValue({ type: "int_pair", value: [p.width, p.height] })
                }
                className={clsx(
                  "text-[11px] px-2 py-1 rounded border transition-all",
                  matchedPreset === p
                    ? "border-blue-400/60 bg-blue-400/15 text-white"
                    : "border-white/10 text-white/60 hover:bg-white/5",
                )}
              >
                {p.label}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-2">
            <input
              type="number"
              value={w}
              onChange={(e) =>
                onValue({ type: "int_pair", value: [Number(e.target.value), h] })
              }
              className="!w-24"
            />
            <span className="text-white/40">×</span>
            <input
              type="number"
              value={h}
              onChange={(e) =>
                onValue({ type: "int_pair", value: [w, Number(e.target.value)] })
              }
              className="!w-24"
            />
            <span className="text-[11px] text-white/40 ml-1">
              {matchedPreset ? "已匹配预设" : "自定义"}
            </span>
          </div>
        </div>
      );
    }
    case "enum": {
      const v = value?.type === "enum" ? value.value : entry.kind.choices[0]?.[0] ?? "";
      return (
        <select
          value={v}
          onChange={(e) => onValue({ type: "enum", value: e.target.value })}
          className="!w-auto"
        >
          {entry.kind.choices.map(([val, label]) => (
            <option key={val} value={val}>
              {label}
            </option>
          ))}
        </select>
      );
    }
    case "enum_args": {
      const v =
        value?.type === "enum"
          ? value.value
          : entry.kind.choices[0]?.value ?? "";
      // Render as a horizontal radio-pill group — clearer than a dropdown
      // for short lists like the window-mode entry.
      return (
        <div className="flex flex-wrap gap-1.5">
          {entry.kind.choices.map((c) => (
            <button
              key={c.value}
              type="button"
              onClick={() => onValue({ type: "enum", value: c.value })}
              className={clsx(
                "text-xs px-3 py-1.5 rounded border transition-all",
                v === c.value
                  ? "border-blue-400/60 bg-blue-400/15 text-white"
                  : "border-white/10 text-white/60 hover:bg-white/5",
              )}
            >
              {c.label_zh}
            </button>
          ))}
        </div>
      );
    }
    case "string": {
      const v = value?.type === "string" ? value.value : "";
      return (
        <input
          type="text"
          value={v}
          placeholder={entry.kind.placeholder}
          onChange={(e) => onValue({ type: "string", value: e.target.value })}
        />
      );
    }
  }
}
