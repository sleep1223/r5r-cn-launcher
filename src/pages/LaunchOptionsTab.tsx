import { useEffect, useMemo, useRef, useState } from "react";
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

export function LaunchOptionsTab() {
  const { settings, update } = useSettings();
  const [catalog, setCatalog] = useState<LaunchOptionCatalog | null>(null);
  const [selection, setSelection] = useState<LaunchOptionSelection>({ items: {} });
  const [composed, setComposed] = useState<string[]>([]);
  const [warnings, setWarnings] = useState<LaunchWarning[]>([]);
  const [copyFlash, setCopyFlash] = useState(false);
  const saveTimer = useRef<number | null>(null);
  const initialLoadDone = useRef(false);

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
            提示：启动项配置会自动保存到本地，首页【启动游戏】会读取这里的设置传给 r5apex.exe。
          </div>
        </GlassCard>
      </div>
    </div>
  );
}

interface EntryRowProps {
  entry: OptionEntry;
  enabled: boolean;
  value: OptionValue | null;
  onToggle: (en: boolean) => void;
  onValue: (v: OptionValue) => void;
}

function EntryRow({ entry, enabled, value, onToggle, onValue }: EntryRowProps) {
  return (
    <div
      className={clsx(
        "rounded-lg border p-3 transition-all",
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
          <div className="text-sm font-medium">{entry.label_zh}</div>
          <div className="text-[11px] text-white/50 mt-0.5 leading-relaxed">
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
    case "int_pair": {
      const [w, h] =
        value?.type === "int_pair" ? value.value : [1920, 1080];
      return (
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
