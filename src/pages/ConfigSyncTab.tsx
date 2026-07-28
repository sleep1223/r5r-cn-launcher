import clsx from "clsx";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import {
  deleteMyGameConfigPreset,
  GameConfigInputDevice,
  GameConfigPreset,
  GameConfigPresetSummary,
  getGameConfigPreset,
  getGameConfigPresets,
  getMyGameConfigPreset,
  saveMyGameConfigPreset,
} from "../api";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { useAuth } from "../hooks/useAuth";
import {
  applyGameConfig,
  createGameConfigBackup,
  deleteGameConfigBackup,
  generateGameConfigContent,
  listGameConfigBackups,
  previewGameConfigApply,
  restoreGameConfigBackup,
  restoreLatestGameConfigBackup,
  scanGameConfigs,
} from "../ipc/configSync";
import {
  ConfigApplyPreview,
  ConfigBackupRecord,
  ConfigComparison,
  ConfigEntryState,
  ConfigGame,
  ConfigSection,
  GameConfigSnapshot,
} from "../ipc/types";

type PageSection = "compare" | "backups" | "community";
type CommunityView = "browse" | "upload";
type GroupFilter = "all" | ConfigSection;

const GAME_LABEL: Record<ConfigGame, string> = { apex: "Apex", r5: "R5" };
const SECTION_LABEL: Record<ConfigSection, string> = {
  mouse_keyboard: "鼠标/键盘",
  controller: "控制器",
  fov: "视野 (FOV)",
};

interface PendingApply {
  content: string;
  selectedKeys: string[];
  targetGames: ConfigGame[];
  preview: ConfigApplyPreview;
  sourceLabel: string;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function snapshotFor(
  comparison: ConfigComparison,
  game: ConfigGame,
): GameConfigSnapshot {
  return game === "apex" ? comparison.apex : comparison.r5;
}

function parseContent(content: string): { key: string; value: string }[] {
  return content
    .split(/\r?\n/)
    .map((line) => line.match(/^([A-Za-z0-9_]+)\s+"([^"]+)"$/))
    .filter((match): match is RegExpMatchArray => match !== null)
    .map((match) => ({ key: match[1], value: match[2] }));
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatDate(value: string | number): string {
  return new Date(value).toLocaleString("zh-CN", { hour12: false });
}

function operationSourceLabel(source: string): string {
  if (source === "manual") return "手动";
  if (source.startsWith("apply:")) return "应用前";
  if (source.startsWith("restore:")) return "恢复前";
  return source;
}

export function ConfigSyncTab() {
  const [section, setSection] = useState<PageSection>("compare");
  const [comparison, setComparison] = useState<ConfigComparison | null>(null);
  const [backups, setBackups] = useState<ConfigBackupRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<
    { kind: "success" | "error"; text: string } | null
  >(null);
  const [pendingApply, setPendingApply] = useState<PendingApply | null>(null);

  const refreshComparison = useCallback(async () => {
    const result = await scanGameConfigs();
    setComparison(result);
    return result;
  }, []);

  const refreshBackups = useCallback(async () => {
    setBackups(await listGameConfigBackups());
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    Promise.all([scanGameConfigs(), listGameConfigBackups()])
      .then(([nextComparison, nextBackups]) => {
        if (!cancelled) {
          setComparison(nextComparison);
          setBackups(nextBackups);
        }
      })
      .catch((error) => {
        if (!cancelled) setNotice({ kind: "error", text: errorMessage(error) });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const beginApply = useCallback(
    async (
      content: string,
      selectedKeys: string[],
      targetGames: ConfigGame[],
      sourceLabel: string,
    ) => {
      setBusy(true);
      setNotice(null);
      try {
        const preview = await previewGameConfigApply(
          content,
          selectedKeys,
          targetGames,
        );
        setPendingApply({
          content,
          selectedKeys,
          targetGames,
          preview,
          sourceLabel,
        });
      } catch (error) {
        setNotice({ kind: "error", text: errorMessage(error) });
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const confirmApply = useCallback(async () => {
    if (!pendingApply) return;
    setBusy(true);
    setNotice(null);
    try {
      const result = await applyGameConfig({
        content: pendingApply.content,
        selected_keys: pendingApply.selectedKeys,
        target_games: pendingApply.targetGames,
        expected_preview: pendingApply.preview,
        source_label: pendingApply.sourceLabel,
      });
      const summary = result.targets
        .map(
          (target) =>
            `${GAME_LABEL[target.game]}：替换 ${target.replaced}，跳过 ${target.unchanged}，缺失 ${target.missing}`,
        )
        .join("；");
      setPendingApply(null);
      await Promise.all([refreshComparison(), refreshBackups()]);
      setNotice({ kind: "success", text: `应用完成。${summary}` });
    } catch (error) {
      setNotice({ kind: "error", text: errorMessage(error) });
    } finally {
      setBusy(false);
    }
  }, [pendingApply, refreshBackups, refreshComparison]);

  if (loading) {
    return <div className="p-8 text-white/60">正在扫描 Apex / R5 配置…</div>;
  }

  return (
    <div className="p-6 space-y-5 min-w-[760px]">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">配置同步</h1>
          <p className="text-xs text-white/45 mt-1">
            对比灵敏度、保存完整快照，并安全应用社区配置。
          </p>
        </div>
        <PrimaryButton
          variant="secondary"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            refreshComparison()
              .then(() => setNotice({ kind: "success", text: "本机配置已重新扫描" }))
              .catch((error) =>
                setNotice({ kind: "error", text: errorMessage(error) }),
              )
              .finally(() => setBusy(false));
          }}
        >
          重新扫描
        </PrimaryButton>
      </header>

      <nav className="flex gap-1 border-b border-white/10" aria-label="配置同步功能">
        {(
          [
            ["compare", "本机对比"],
            ["backups", "本地备份"],
            ["community", "社区配置"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            onClick={() => setSection(id)}
            className={clsx(
              "px-4 py-2.5 text-sm border-b-2 -mb-px transition-colors",
              section === id
                ? "border-blue-400 text-white"
                : "border-transparent text-white/50 hover:text-white/80",
            )}
          >
            {label}
          </button>
        ))}
      </nav>

      {notice && (
        <div
          className={clsx(
            "rounded-lg border px-4 py-3 text-sm",
            notice.kind === "success"
              ? "border-emerald-400/25 bg-emerald-400/8 text-emerald-200"
              : "border-red-400/25 bg-red-400/8 text-red-200",
          )}
          role="status"
        >
          {notice.text}
        </div>
      )}

      {comparison && section === "compare" && (
        <LocalComparison
          comparison={comparison}
          busy={busy}
          onSync={async (source, target, keys) => {
            setBusy(true);
            setNotice(null);
            try {
              const generated = await generateGameConfigContent(source, keys);
              await beginApply(
                generated.content,
                generated.keys,
                [target],
                `${GAME_LABEL[source]} → ${GAME_LABEL[target]}`,
              );
            } catch (error) {
              setNotice({ kind: "error", text: errorMessage(error) });
            } finally {
              setBusy(false);
            }
          }}
        />
      )}

      {section === "backups" && comparison && (
        <BackupManager
          comparison={comparison}
          backups={backups}
          busy={busy}
          onChanged={async (message) => {
            await Promise.all([refreshBackups(), refreshComparison()]);
            setNotice({ kind: "success", text: message });
          }}
          onError={(error) => setNotice({ kind: "error", text: errorMessage(error) })}
          setBusy={setBusy}
        />
      )}

      {section === "community" && comparison && (
        <CommunityConfigs
          comparison={comparison}
          busy={busy}
          setBusy={setBusy}
          onApply={beginApply}
          onNotice={setNotice}
        />
      )}

      {pendingApply && (
        <ApplyPreviewDialog
          pending={pendingApply}
          busy={busy}
          onCancel={() => setPendingApply(null)}
          onConfirm={confirmApply}
        />
      )}
    </div>
  );
}

function LocalComparison({
  comparison,
  busy,
  onSync,
}: {
  comparison: ConfigComparison;
  busy: boolean;
  onSync: (source: ConfigGame, target: ConfigGame, keys: string[]) => Promise<void>;
}) {
  const [filter, setFilter] = useState<GroupFilter>("all");
  const apexByKey = useMemo(
    () => new Map(comparison.apex.entries.map((entry) => [entry.key, entry])),
    [comparison.apex.entries],
  );
  const r5ByKey = useMemo(
    () => new Map(comparison.r5.entries.map((entry) => [entry.key, entry])),
    [comparison.r5.entries],
  );
  const rows = comparison.apex.entries.filter(
    (entry) => filter === "all" || entry.section === filter,
  );

  const syncKeys = (source: ConfigGame) => {
    const sourceMap = source === "apex" ? apexByKey : r5ByKey;
    return rows
      .map((row) => sourceMap.get(row.key))
      .filter((entry): entry is ConfigEntryState => Boolean(entry?.present && !entry.conflict))
      .map((entry) => entry.key);
  };

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <DetectionSummary snapshot={comparison.apex} />
        <DetectionSummary snapshot={comparison.r5} />
      </div>

      <GlassCard padding={false}>
        <div className="p-4 flex flex-wrap items-center justify-between gap-3 border-b border-white/8">
          <div className="flex gap-1.5">
            {(
              [
                ["all", "全部"],
                ["mouse_keyboard", SECTION_LABEL.mouse_keyboard],
                ["controller", SECTION_LABEL.controller],
                ["fov", SECTION_LABEL.fov],
              ] as const
            ).map(([id, label]) => (
              <FilterPill key={id} active={filter === id} onClick={() => setFilter(id)}>
                {label}
              </FilterPill>
            ))}
          </div>
          <div className="flex gap-2">
            <PrimaryButton
              variant="secondary"
              disabled={busy || !comparison.apex.detected || !comparison.r5.detected}
              onClick={() => void onSync("apex", "r5", syncKeys("apex"))}
            >
              Apex → R5
            </PrimaryButton>
            <PrimaryButton
              variant="secondary"
              disabled={busy || !comparison.apex.detected || !comparison.r5.detected}
              onClick={() => void onSync("r5", "apex", syncKeys("r5"))}
            >
              R5 → Apex
            </PrimaryButton>
          </div>
        </div>

        <div className="overflow-auto max-h-[560px]">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-[#171b20] z-10 text-[11px] text-white/45">
              <tr className="border-b border-white/8">
                <th className="text-left px-4 py-2.5 w-[34%]">配置项</th>
                <th className="text-left px-3 py-2.5">Apex</th>
                <th className="text-left px-3 py-2.5">R5</th>
                <th className="text-right px-4 py-2.5">状态</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => {
                const apex = apexByKey.get(row.key)!;
                const r5 = r5ByKey.get(row.key)!;
                const status = compareStatus(apex, r5);
                return (
                  <tr key={row.key} className="border-b border-white/[0.045] hover:bg-white/[0.025]">
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-2">
                        <GroupBadge section={row.section} />
                        <span className="font-medium">{row.label}</span>
                      </div>
                      <div className="font-mono text-[10px] text-white/30 mt-1">{row.key}</div>
                    </td>
                    <ConfigValueCell entry={apex} />
                    <ConfigValueCell entry={r5} />
                    <td className="px-4 py-2.5 text-right">
                      <StatusBadge status={status} />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </GlassCard>
    </div>
  );
}

function DetectionSummary({ snapshot }: { snapshot: GameConfigSnapshot }) {
  return (
    <div className="rounded-xl border border-white/8 bg-white/[0.025] px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <span className="font-medium">{GAME_LABEL[snapshot.game]}</span>
        <span
          className={clsx(
            "text-[10px] rounded-full px-2 py-0.5",
            snapshot.detected
              ? "bg-emerald-400/12 text-emerald-300"
              : "bg-white/8 text-white/40",
          )}
        >
          {snapshot.detected ? `已检测 · ${snapshot.files.length} 个文件` : "未检测到"}
        </span>
      </div>
      <div className="text-[10px] font-mono text-white/35 truncate mt-1.5" title={snapshot.root_path}>
        {snapshot.root_path}
      </div>
    </div>
  );
}

function compareStatus(left: ConfigEntryState, right: ConfigEntryState) {
  if (left.conflict || right.conflict) return "conflict" as const;
  if (!left.present || !right.present) return "missing" as const;
  return left.value === right.value ? ("same" as const) : ("different" as const);
}

function ConfigValueCell({ entry }: { entry: ConfigEntryState }) {
  return (
    <td className="px-3 py-2.5 font-mono text-xs tabular-nums">
      {entry.conflict ? (
        <span className="text-red-300" title={entry.values.join(", ")}>
          多值冲突 ({entry.values.length})
        </span>
      ) : entry.present ? (
        <span className="text-white/85">{entry.value}</span>
      ) : (
        <span className="text-white/25">—</span>
      )}
    </td>
  );
}

function StatusBadge({ status }: { status: "same" | "different" | "missing" | "conflict" }) {
  const styles = {
    same: "bg-emerald-400/10 text-emerald-300",
    different: "bg-amber-400/10 text-amber-300",
    missing: "bg-white/7 text-white/45",
    conflict: "bg-red-400/10 text-red-300",
  };
  const labels = { same: "相同", different: "不同", missing: "缺失", conflict: "冲突" };
  return <span className={clsx("text-[10px] rounded px-2 py-1", styles[status])}>{labels[status]}</span>;
}

function BackupManager({
  comparison,
  backups,
  busy,
  setBusy,
  onChanged,
  onError,
}: {
  comparison: ConfigComparison;
  backups: ConfigBackupRecord[];
  busy: boolean;
  setBusy: (value: boolean) => void;
  onChanged: (message: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const act = async (action: () => Promise<unknown>, message: string) => {
    setBusy(true);
    try {
      await action();
      await onChanged(message);
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-2 gap-4">
        {([comparison.apex, comparison.r5] as const).map((snapshot) => (
          <GlassCard key={snapshot.game}>
            <SectionHeader
              title={`${GAME_LABEL[snapshot.game]} 备份`}
              subtitle={snapshot.detected ? "完整保存 settings.cfg 与 profile.cfg" : "未检测到有效配置文件"}
            />
            <div className="flex gap-2">
              <PrimaryButton
                disabled={busy || !snapshot.detected}
                onClick={() =>
                  void act(
                    () => createGameConfigBackup(snapshot.game),
                    `${GAME_LABEL[snapshot.game]} 手动备份已创建`,
                  )
                }
              >
                立即备份
              </PrimaryButton>
              <PrimaryButton
                variant="secondary"
                disabled={busy || !backups.some((backup) => backup.game === snapshot.game)}
                onClick={() => {
                  if (!window.confirm(`恢复 ${GAME_LABEL[snapshot.game]} 的最近备份？当前配置会先自动备份。`)) return;
                  void act(
                    () => restoreLatestGameConfigBackup(snapshot.game),
                    `${GAME_LABEL[snapshot.game]} 已恢复最近备份`,
                  );
                }}
              >
                恢复上次
              </PrimaryButton>
            </div>
          </GlassCard>
        ))}
      </div>

      <GlassCard padding={false}>
        <div className="px-5 py-4 border-b border-white/8">
          <div className="font-medium">备份历史</div>
          <div className="text-xs text-white/45 mt-0.5">无限保留；恢复前会再次备份当前文件。</div>
        </div>
        {backups.length === 0 ? (
          <EmptyState text="还没有本地备份" />
        ) : (
          <div className="divide-y divide-white/[0.055] max-h-[520px] overflow-y-auto">
            {backups.map((backup) => (
              <div key={backup.id} className="px-5 py-3 flex items-center gap-4 hover:bg-white/[0.02]">
                <span className="text-xs font-semibold w-10">{GAME_LABEL[backup.game]}</span>
                <div className="flex-1 min-w-0">
                  <div className="text-sm truncate">{backup.label}</div>
                  <div className="text-[10px] text-white/35 mt-1">
                    {formatDate(backup.created_at_ms)} · {backup.files.length} 个文件 · {formatBytes(backup.total_size)} · {operationSourceLabel(backup.operation_source)}
                  </div>
                </div>
                <PrimaryButton
                  variant="secondary"
                  disabled={busy}
                  onClick={() => {
                    if (!window.confirm("恢复这个备份？当前配置会先自动备份。")) return;
                    void act(
                      () => restoreGameConfigBackup(backup.id),
                      `${GAME_LABEL[backup.game]} 备份已恢复`,
                    );
                  }}
                >
                  恢复
                </PrimaryButton>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    if (!window.confirm("永久删除这个本地备份？")) return;
                    void act(
                      () => deleteGameConfigBackup(backup.id),
                      "备份已删除",
                    );
                  }}
                  className="text-xs text-red-300/70 hover:text-red-200 disabled:opacity-40 px-2 py-2"
                >
                  删除
                </button>
              </div>
            ))}
          </div>
        )}
      </GlassCard>
    </div>
  );
}

function CommunityConfigs({
  comparison,
  busy,
  setBusy,
  onApply,
  onNotice,
}: {
  comparison: ConfigComparison;
  busy: boolean;
  setBusy: (value: boolean) => void;
  onApply: (
    content: string,
    selectedKeys: string[],
    targetGames: ConfigGame[],
    sourceLabel: string,
  ) => Promise<void>;
  onNotice: (notice: { kind: "success" | "error"; text: string } | null) => void;
}) {
  const [view, setView] = useState<CommunityView>("browse");
  const [reloadToken, setReloadToken] = useState(0);

  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        <FilterPill active={view === "browse"} onClick={() => setView("browse")}>
          公开配置
        </FilterPill>
        <FilterPill active={view === "upload"} onClick={() => setView("upload")}>
          管理我的上传
        </FilterPill>
      </div>

      {view === "browse" ? (
        <CommunityBrowser
          comparison={comparison}
          busy={busy}
          onApply={onApply}
          reloadToken={reloadToken}
          onNotice={onNotice}
        />
      ) : (
        <UploadManager
          comparison={comparison}
          busy={busy}
          setBusy={setBusy}
          onSaved={() => setReloadToken((value) => value + 1)}
          onNotice={onNotice}
        />
      )}
    </div>
  );
}

function CommunityBrowser({
  comparison,
  busy,
  onApply,
  reloadToken,
  onNotice,
}: {
  comparison: ConfigComparison;
  busy: boolean;
  onApply: (
    content: string,
    selectedKeys: string[],
    targetGames: ConfigGame[],
    sourceLabel: string,
  ) => Promise<void>;
  reloadToken: number;
  onNotice: (notice: { kind: "success" | "error"; text: string } | null) => void;
}) {
  const [draftQuery, setDraftQuery] = useState("");
  const [query, setQuery] = useState("");
  const [device, setDevice] = useState<GameConfigInputDevice | "all">("all");
  const [page, setPage] = useState(1);
  const [items, setItems] = useState<GameConfigPresetSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<GameConfigPreset | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getGameConfigPresets({
      q: query || undefined,
      input_device: device === "all" ? undefined : device,
      page_no: page,
      page_size: 20,
    })
      .then((result) => {
        if (!cancelled) {
          setItems(result.data);
          setTotal(result.total);
          if (detail && !result.data.some((item) => item.id === detail.id)) setDetail(null);
        }
      })
      .catch((error) => {
        if (!cancelled) onNotice({ kind: "error", text: errorMessage(error) });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [detail, device, onNotice, page, query, reloadToken]);

  const selectPreset = async (id: number) => {
    setDetailLoading(true);
    onNotice(null);
    try {
      setDetail(await getGameConfigPreset(id));
    } catch (error) {
      onNotice({ kind: "error", text: errorMessage(error) });
    } finally {
      setDetailLoading(false);
    }
  };

  return (
    <div className="grid grid-cols-[minmax(300px,0.9fr)_minmax(420px,1.35fr)] gap-4 items-start">
      <GlassCard padding={false}>
        <form
          className="p-4 border-b border-white/8 space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            setPage(1);
            setQuery(draftQuery.trim());
          }}
        >
          <div className="flex gap-2">
            <input
              type="text"
              value={draftQuery}
              onChange={(event) => setDraftQuery(event.target.value)}
              placeholder="搜索名称或上传者"
              aria-label="搜索社区配置"
            />
            <PrimaryButton type="submit" disabled={loading}>搜索</PrimaryButton>
          </div>
          <div className="flex gap-1.5">
            {(
              [
                ["all", "全部"],
                ["mouse_keyboard", `包含${SECTION_LABEL.mouse_keyboard}`],
                ["controller", `包含${SECTION_LABEL.controller}`],
              ] as const
            ).map(([id, label]) => (
              <FilterPill
                key={id}
                active={device === id}
                onClick={() => {
                  setDevice(id);
                  setPage(1);
                }}
              >
                {label}
              </FilterPill>
            ))}
          </div>
        </form>

        {loading ? (
          <EmptyState text="正在加载社区配置…" />
        ) : items.length === 0 ? (
          <EmptyState text="没有匹配的社区配置" />
        ) : (
          <div className="divide-y divide-white/[0.055] max-h-[500px] overflow-y-auto">
            {items.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => void selectPreset(item.id)}
                className={clsx(
                  "w-full text-left px-4 py-3 hover:bg-white/[0.035] transition-colors",
                  detail?.id === item.id && "bg-blue-400/[0.07]",
                )}
              >
                <div className="flex justify-between gap-3">
                  <span className="font-medium truncate">{item.name}</span>
                  <span className="text-[10px] text-white/30 shrink-0">{GAME_LABEL[item.source_game]}</span>
                </div>
                <div className="text-xs text-white/45 mt-1 truncate">{item.creator_name}</div>
                <div className="flex gap-1 mt-2">
                  <PresetBadges preset={item} />
                </div>
              </button>
            ))}
          </div>
        )}
        <div className="px-4 py-3 flex justify-between items-center border-t border-white/8 text-xs text-white/40">
          <span>共 {total} 份</span>
          <div className="flex items-center gap-2">
            <button disabled={page <= 1} onClick={() => setPage((value) => value - 1)} className="disabled:opacity-25 hover:text-white">上一页</button>
            <span>{page}</span>
            <button disabled={page * 20 >= total} onClick={() => setPage((value) => value + 1)} className="disabled:opacity-25 hover:text-white">下一页</button>
          </div>
        </div>
      </GlassCard>

      {detailLoading ? (
        <GlassCard><EmptyState text="正在读取配置详情…" /></GlassCard>
      ) : detail ? (
        <CommunityDetail
          detail={detail}
          comparison={comparison}
          busy={busy}
          onApply={onApply}
        />
      ) : (
        <GlassCard><EmptyState text="选择一份配置查看并应用" /></GlassCard>
      )}
    </div>
  );
}

function CommunityDetail({
  detail,
  comparison,
  busy,
  onApply,
}: {
  detail: GameConfigPreset;
  comparison: ConfigComparison;
  busy: boolean;
  onApply: (
    content: string,
    selectedKeys: string[],
    targetGames: ConfigGame[],
    sourceLabel: string,
  ) => Promise<void>;
}) {
  const rows = useMemo(() => parseContent(detail.content), [detail.content]);
  const labelMap = useMemo(
    () =>
      new Map(
        [...comparison.apex.entries, ...comparison.r5.entries].map((entry) => [
          entry.key,
          { label: entry.label, section: entry.section },
        ]),
      ),
    [comparison],
  );
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(
    () => new Set(rows.map((row) => row.key)),
  );
  const detectedTargets = ([comparison.apex, comparison.r5] as const)
    .filter((snapshot) => snapshot.detected)
    .map((snapshot) => snapshot.game);
  const [targets, setTargets] = useState<Set<ConfigGame>>(
    () => new Set(detectedTargets),
  );

  useEffect(() => {
    setSelectedKeys(new Set(rows.map((row) => row.key)));
    setTargets(new Set(detectedTargets));
  }, [detail.id]);

  const groupKeys = (section: ConfigSection) =>
    rows
      .filter((row) => labelMap.get(row.key)?.section === section)
      .map((row) => row.key);

  return (
    <GlassCard>
      <SectionHeader
        title={detail.name}
        subtitle={`上传者 ${detail.creator_name} · 来源 ${GAME_LABEL[detail.source_game]} · 更新于 ${formatDate(detail.updated_at)}`}
        right={<div className="flex gap-1"><PresetBadges preset={detail} /></div>}
      />
      {detail.remark && (
        <p className="text-xs text-white/55 leading-relaxed mb-4 whitespace-pre-wrap">{detail.remark}</p>
      )}

      <div className="flex flex-wrap gap-1.5 mb-3">
        {(["mouse_keyboard", "controller", "fov"] as const).map((section) => {
          const keys = groupKeys(section);
          if (keys.length === 0) return null;
          const allSelected = keys.every((key) => selectedKeys.has(key));
          return (
            <FilterPill
              key={section}
              active={allSelected}
              onClick={() => setSelectedKeys(toggleGroup(selectedKeys, keys, !allSelected))}
            >
              {SECTION_LABEL[section]} {allSelected ? "已全选" : "全选"}
            </FilterPill>
          );
        })}
      </div>

      <div className="border border-white/8 rounded-lg max-h-[300px] overflow-y-auto divide-y divide-white/[0.045]">
        {rows.map((row) => {
          const meta = labelMap.get(row.key);
          return (
            <label key={row.key} className="flex items-center gap-3 px-3 py-2 hover:bg-white/[0.025] cursor-pointer">
              <input
                type="checkbox"
                checked={selectedKeys.has(row.key)}
                onChange={(event) => setSelectedKeys(toggleOne(selectedKeys, row.key, event.target.checked))}
                className="size-4 accent-blue-400"
              />
              {meta && <GroupBadge section={meta.section} />}
              <div className="flex-1 min-w-0">
                <div className="text-xs">{meta?.label ?? row.key}</div>
                <div className="font-mono text-[10px] text-white/30 truncate">{row.key}</div>
              </div>
              <span className="font-mono text-xs text-blue-200 tabular-nums">{row.value}</span>
            </label>
          );
        })}
      </div>

      <div className="mt-4">
        <div className="text-xs text-white/45 mb-2">应用目标（可多选）</div>
        <div className="flex gap-2">
          {detectedTargets.map((game) => (
            <FilterPill
              key={game}
              active={targets.has(game)}
              onClick={() => setTargets(toggleOne(targets, game, !targets.has(game)))}
            >
              {GAME_LABEL[game]}
            </FilterPill>
          ))}
          {detectedTargets.length === 0 && <span className="text-xs text-white/35">未检测到可应用的本机配置</span>}
        </div>
      </div>

      <PrimaryButton
        className="mt-4"
        disabled={busy || selectedKeys.size === 0 || targets.size === 0}
        onClick={() =>
          void onApply(
            detail.content,
            [...selectedKeys],
            [...targets],
            `社区配置：${detail.name}（${detail.creator_name}）`,
          )
        }
      >
        预览应用
      </PrimaryButton>
    </GlassCard>
  );
}

function UploadManager({
  comparison,
  busy,
  setBusy,
  onSaved,
  onNotice,
}: {
  comparison: ConfigComparison;
  busy: boolean;
  setBusy: (value: boolean) => void;
  onSaved: () => void;
  onNotice: (notice: { kind: "success" | "error"; text: string } | null) => void;
}) {
  const { appKey } = useAuth();
  const detectedGames = ([comparison.apex, comparison.r5] as const).filter(
    (snapshot) => snapshot.detected,
  );
  const [sourceGame, setSourceGame] = useState<ConfigGame>(
    detectedGames[0]?.game ?? "apex",
  );
  const source = snapshotFor(comparison, sourceGame);
  const available = source.entries.filter((entry) => entry.present);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const [name, setName] = useState("");
  const [remark, setRemark] = useState("");
  const [mine, setMine] = useState<GameConfigPreset | null>(null);
  const [mineLoading, setMineLoading] = useState(false);

  useEffect(() => {
    if (!appKey) {
      setMine(null);
      return;
    }
    let cancelled = false;
    setMineLoading(true);
    getMyGameConfigPreset(appKey)
      .then((preset) => {
        if (cancelled) return;
        setMine(preset);
        setName(preset.name);
        setRemark(preset.remark ?? "");
        const requestedSource = snapshotFor(comparison, preset.source_game);
        const nextSource = requestedSource.detected
          ? preset.source_game
          : detectedGames[0]?.game ?? preset.source_game;
        setSourceGame(nextSource);
        const availableKeys = new Set(
          snapshotFor(comparison, nextSource)
            .entries.filter((entry) => entry.present && !entry.conflict)
            .map((entry) => entry.key),
        );
        setSelectedKeys(
          new Set(
            parseContent(preset.content)
              .map((row) => row.key)
              .filter((key) => availableKeys.has(key)),
          ),
        );
      })
      .catch((error) => {
        if (!cancelled && errorMessage(error).includes("尚未上传配置")) {
          setMine(null);
          setName("");
          setRemark("");
        } else if (!cancelled) {
          onNotice({ kind: "error", text: errorMessage(error) });
        }
      })
      .finally(() => {
        if (!cancelled) setMineLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [appKey, comparison, onNotice]);

  useEffect(() => {
    const availableKeys = new Set(
      available.filter((entry) => !entry.conflict).map((entry) => entry.key),
    );
    setSelectedKeys((current) => {
      const retained = [...current].filter((key) => availableKeys.has(key));
      return new Set(retained.length > 0 ? retained : availableKeys);
    });
  }, [sourceGame]);

  if (!appKey) {
    return (
      <GlassCard>
        <EmptyState text="上传配置需要先使用 AppKey 登录；请点击左下角的「登录」。" />
      </GlassCard>
    );
  }
  if (mineLoading) return <GlassCard><EmptyState text="正在读取你的配置…" /></GlassCard>;
  if (detectedGames.length === 0) {
    return <GlassCard><EmptyState text="未检测到 Apex 或 R5 配置，暂时无法上传。" /></GlassCard>;
  }

  const groupKeys = (section: ConfigSection) =>
    available
      .filter((entry) => entry.section === section && !entry.conflict)
      .map((entry) => entry.key);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!name.trim() || selectedKeys.size === 0) return;
    setBusy(true);
    onNotice(null);
    try {
      const generated = await generateGameConfigContent(sourceGame, [...selectedKeys]);
      const saved = await saveMyGameConfigPreset(appKey, {
        name: name.trim(),
        remark: remark.trim() || null,
        source_game: sourceGame,
        content: generated.content,
      });
      setMine(saved);
      onSaved();
      onNotice({ kind: "success", text: mine ? "你的社区配置已更新" : "社区配置已上传" });
    } catch (error) {
      onNotice({ kind: "error", text: errorMessage(error) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={(event) => void submit(event)} className="grid grid-cols-[320px_1fr] gap-4 items-start">
      <GlassCard>
        <SectionHeader
          title={mine ? "更新我的配置" : "上传配置"}
          subtitle="每位上传者仅保留一份，保存时会覆盖更新。"
        />
        <label className="block text-xs text-white/55 mb-1.5" htmlFor="preset-name">名称 *</label>
        <input
          id="preset-name"
          type="text"
          maxLength={64}
          required
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="例如：低灵敏度全瞄具配置"
        />
        <label className="block text-xs text-white/55 mb-1.5 mt-4" htmlFor="preset-remark">备注</label>
        <textarea
          id="preset-remark"
          rows={5}
          maxLength={500}
          value={remark}
          onChange={(event) => setRemark(event.target.value)}
          placeholder="可选：说明配置风格或使用建议"
        />
        <div className="text-xs text-white/55 mt-4 mb-2">唯一数值来源</div>
        <div className="flex gap-2">
          {detectedGames.map((snapshot) => (
            <FilterPill
              key={snapshot.game}
              active={sourceGame === snapshot.game}
              onClick={() => setSourceGame(snapshot.game)}
            >
              {GAME_LABEL[snapshot.game]}
            </FilterPill>
          ))}
        </div>
        <div className="flex gap-2 mt-5">
          <PrimaryButton type="submit" disabled={busy || !name.trim() || selectedKeys.size === 0}>
            {mine ? "保存更新" : "上传"}
          </PrimaryButton>
          {mine && (
            <PrimaryButton
              type="button"
              variant="danger"
              disabled={busy}
              onClick={() => {
                if (!window.confirm("删除你唯一的社区配置？")) return;
                setBusy(true);
                deleteMyGameConfigPreset(appKey)
                  .then(() => {
                    setMine(null);
                    setName("");
                    setRemark("");
                    onSaved();
                    onNotice({ kind: "success", text: "社区配置已删除" });
                  })
                  .catch((error) => onNotice({ kind: "error", text: errorMessage(error) }))
                  .finally(() => setBusy(false));
              }}
            >
              删除
            </PrimaryButton>
          )}
        </div>
      </GlassCard>

      <GlassCard padding={false}>
        <div className="p-4 border-b border-white/8">
          <div className="font-medium">选择公开的配置项</div>
          <div className="text-xs text-white/45 mt-1">仅展示来源游戏中实际存在的字段；冲突字段会标红且无法生成。</div>
          <div className="flex flex-wrap gap-1.5 mt-3">
            {(["mouse_keyboard", "controller", "fov"] as const).map((section) => {
              const keys = groupKeys(section);
              if (keys.length === 0) return null;
              const allSelected = keys.every((key) => selectedKeys.has(key));
              return (
                <FilterPill
                  key={section}
                  active={allSelected}
                  onClick={() => setSelectedKeys(toggleGroup(selectedKeys, keys, !allSelected))}
                >
                  {SECTION_LABEL[section]} {allSelected ? "取消全选" : "全选"}
                </FilterPill>
              );
            })}
          </div>
        </div>
        <div className="max-h-[520px] overflow-y-auto divide-y divide-white/[0.045]">
          {available.map((entry) => (
            <label
              key={entry.key}
              className={clsx(
                "flex items-center gap-3 px-4 py-2.5",
                entry.conflict ? "bg-red-400/[0.045] cursor-not-allowed" : "hover:bg-white/[0.025] cursor-pointer",
              )}
            >
              <input
                type="checkbox"
                disabled={entry.conflict}
                checked={selectedKeys.has(entry.key) && !entry.conflict}
                onChange={(event) => setSelectedKeys(toggleOne(selectedKeys, entry.key, event.target.checked))}
                className="size-4 accent-blue-400"
              />
              <GroupBadge section={entry.section} />
              <div className="flex-1 min-w-0">
                <div className="text-xs">{entry.label}</div>
                <div className="font-mono text-[10px] text-white/30 truncate">{entry.key}</div>
              </div>
              <span className={clsx("font-mono text-xs", entry.conflict ? "text-red-300" : "text-blue-200")}>
                {entry.conflict ? "多值冲突" : entry.value}
              </span>
            </label>
          ))}
        </div>
      </GlassCard>
    </form>
  );
}

function ApplyPreviewDialog({
  pending,
  busy,
  onCancel,
  onConfirm,
}: {
  pending: PendingApply;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
}) {
  const replacements = pending.preview.targets.reduce(
    (sum, target) => sum + target.replace_count,
    0,
  );
  return (
    <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-8">
      <div className="glass w-full max-w-3xl max-h-[85vh] overflow-y-auto p-5 shadow-2xl">
        <SectionHeader
          title="应用预览"
          subtitle="确认后会先完整备份；多目标任一失败时会整体回滚。"
        />
        <div className="grid grid-cols-2 gap-3">
          {pending.preview.targets.map((target) => (
            <div key={target.game} className="rounded-xl border border-white/8 bg-black/15 p-4">
              <div className="font-medium mb-3">{GAME_LABEL[target.game]}</div>
              <div className="grid grid-cols-4 gap-2 text-center">
                <PreviewMetric label="将替换" value={target.replace_count} tone="blue" />
                <PreviewMetric label="值相同" value={target.unchanged_count} tone="green" />
                <PreviewMetric label="字段缺失" value={target.missing_count} tone="muted" />
                <PreviewMetric label="目标冲突" value={target.conflict_count} tone="red" />
              </div>
              <div className="mt-3 max-h-48 overflow-y-auto divide-y divide-white/[0.04]">
                {target.items.map((item) => (
                  <div key={item.key} className="py-1.5 flex items-center gap-3 text-[11px]">
                    <span className="flex-1 truncate" title={item.key}>{item.label}</span>
                    <span className="font-mono text-white/35 truncate max-w-24">
                      {item.current_values.join(" / ") || "—"}
                    </span>
                    <span className="text-white/25">→</span>
                    <span className="font-mono text-blue-200">{item.desired}</span>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
        <div className="flex justify-end gap-2 mt-5">
          <PrimaryButton variant="secondary" disabled={busy} onClick={onCancel}>取消</PrimaryButton>
          <PrimaryButton disabled={busy || replacements === 0} onClick={() => void onConfirm()}>
            {busy ? "应用中…" : replacements === 0 ? "没有需要替换的值" : `确认替换 ${replacements} 项`}
          </PrimaryButton>
        </div>
      </div>
    </div>
  );
}

function PreviewMetric({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone: "blue" | "green" | "muted" | "red";
}) {
  return (
    <div className="rounded-lg bg-white/[0.035] py-2">
      <div
        className={clsx(
          "text-lg font-semibold tabular-nums",
          tone === "blue" && "text-blue-300",
          tone === "green" && "text-emerald-300",
          tone === "muted" && "text-white/45",
          tone === "red" && "text-red-300",
        )}
      >
        {value}
      </div>
      <div className="text-[10px] text-white/35">{label}</div>
    </div>
  );
}

function PresetBadges({ preset }: { preset: Pick<GameConfigPresetSummary, "has_mouse" | "has_controller" | "has_fov"> }) {
  return (
    <>
      {preset.has_mouse && <GroupBadge section="mouse_keyboard" />}
      {preset.has_controller && <GroupBadge section="controller" />}
      {preset.has_fov && <GroupBadge section="fov" />}
    </>
  );
}

function GroupBadge({ section }: { section: ConfigSection }) {
  const styles: Record<ConfigSection, string> = {
    mouse_keyboard: "bg-blue-400/10 text-blue-300",
    controller: "bg-purple-400/10 text-purple-300",
    fov: "bg-amber-400/10 text-amber-300",
  };
  return (
    <span className={clsx("text-[9px] rounded px-1.5 py-0.5 shrink-0", styles[section])}>
      {SECTION_LABEL[section]}
    </span>
  );
}

function FilterPill({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={clsx(
        "text-xs px-3 py-1.5 rounded-full border transition-colors",
        active
          ? "border-blue-400/50 bg-blue-400/12 text-blue-100"
          : "border-white/10 text-white/50 hover:text-white/80 hover:bg-white/5",
      )}
    >
      {children}
    </button>
  );
}

function EmptyState({ text }: { text: string }) {
  return <div className="py-12 px-5 text-center text-sm text-white/35">{text}</div>;
}

function toggleGroup<T>(current: Set<T>, keys: T[], checked: boolean): Set<T> {
  const next = new Set(current);
  keys.forEach((key) => (checked ? next.add(key) : next.delete(key)));
  return next;
}

function toggleOne<T>(current: Set<T>, key: T, checked: boolean): Set<T> {
  return toggleGroup(current, [key], checked);
}
