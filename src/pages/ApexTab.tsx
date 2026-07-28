import {
  FormEvent,
  ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  ApexCachePayload,
  ApexMapMode,
  ApexMapRotation,
  ApexPlatform,
  ApexPlayerStats,
  ApexPredator,
  ApexServerStatusRow,
  ApexServerStatusSection,
  ApexTranslations,
  getApexMapRotation,
  getApexPlayer,
  getApexPredator,
  getApexServerStatus,
  getApexTranslations,
} from "../api";
import { GlassCard } from "../components/GlassCard";

type ApexView = "player" | "maps" | "servers" | "predator";
type LookupMode = "name" | "uid";

interface ApexHistoryItem {
  id: string;
  queried_at: string;
  uid: string;
  player_name: string;
  platform: ApexPlatform;
  level: number | null;
  rank_score: number | null;
  rank_name: string | null;
  rank_div: number | null;
}

const HISTORY_STORAGE_KEY = "r5-apex-player-query-history";
const LAST_UID_STORAGE_KEY = "r5-apex-last-player-uid";
const HISTORY_LIMIT = 50;
const PLATFORMS: ApexPlatform[] = ["PC", "PS4", "X1", "SWITCH"];

const VIEW_OPTIONS: Array<{
  value: ApexView;
  label: string;
  icon: string;
}> = [
  { value: "player", label: "玩家", icon: "◎" },
  { value: "maps", label: "地图轮换", icon: "▱" },
  { value: "servers", label: "服务器状态", icon: "⌁" },
  { value: "predator", label: "顶猎分数", icon: "★" },
];

const PLATFORM_LABELS: Record<ApexPlatform, string> = {
  PC: "PC",
  PS4: "PlayStation",
  X1: "Xbox",
  SWITCH: "Nintendo Switch",
};

function isApexPlatform(value: unknown): value is ApexPlatform {
  return (
    typeof value === "string" &&
    PLATFORMS.includes(value as ApexPlatform)
  );
}

function toNullableNumber(value: unknown): number | null {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function formatNumber(value: unknown): string {
  const parsed = Number(value ?? 0);
  return new Intl.NumberFormat("zh-CN").format(
    Number.isFinite(parsed) ? parsed : 0,
  );
}

function formatDate(value?: string | null): string {
  if (!value) return "—";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime())
    ? value
    : parsed.toLocaleString("zh-CN");
}

function isTrue(value: unknown): boolean {
  return value === true || value === "true" || value === "是";
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

export function ApexTab() {
  const [activeView, setActiveView] = useState<ApexView>("player");
  const [lookupMode, setLookupMode] = useState<LookupMode>("name");
  const [playerQuery, setPlayerQuery] = useState("");
  const [platform, setPlatform] = useState<ApexPlatform>("PC");
  const [playerLoading, setPlayerLoading] = useState(false);
  const [playerError, setPlayerError] = useState<string | null>(null);
  const [playerData, setPlayerData] = useState<ApexPlayerStats | null>(null);
  const [queryHistory, setQueryHistory] = useState<ApexHistoryItem[]>([]);

  const [mapLoading, setMapLoading] = useState(false);
  const [mapError, setMapError] = useState<string | null>(null);
  const [mapCache, setMapCache] =
    useState<ApexCachePayload<ApexMapRotation> | null>(null);

  const [serverLoading, setServerLoading] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const [serverCache, setServerCache] = useState<
    ApexCachePayload<ApexServerStatusSection[]> | null
  >(null);

  const [predatorLoading, setPredatorLoading] = useState(false);
  const [predatorError, setPredatorError] = useState<string | null>(null);
  const [predatorCache, setPredatorCache] =
    useState<ApexCachePayload<ApexPredator> | null>(null);
  const [translations, setTranslations] = useState<ApexTranslations>({});
  const bootLoadFiredRef = useRef(false);

  const translateApexText = useCallback(
    (value?: unknown, serverChinese?: unknown): string => {
      if (value === null || value === undefined || value === "") {
        return serverChinese === null ||
          serverChinese === undefined ||
          serverChinese === ""
          ? "—"
          : String(serverChinese);
      }
      const source = String(value);
      return (
        translations.zh?.[source] ||
        (serverChinese === null ||
        serverChinese === undefined ||
        serverChinese === ""
          ? source
          : String(serverChinese))
      );
    },
    [translations],
  );

  const fetchMaps = useCallback(async () => {
    setMapLoading(true);
    setMapError(null);
    try {
      setMapCache(await getApexMapRotation());
    } catch (error) {
      setMapError(errorMessage(error, "地图轮换加载失败"));
    } finally {
      setMapLoading(false);
    }
  }, []);

  const fetchServerStatus = useCallback(async () => {
    setServerLoading(true);
    setServerError(null);
    try {
      setServerCache(await getApexServerStatus());
    } catch (error) {
      setServerError(errorMessage(error, "官方服务器状态加载失败"));
    } finally {
      setServerLoading(false);
    }
  }, []);

  const fetchPredator = useCallback(async () => {
    setPredatorLoading(true);
    setPredatorError(null);
    try {
      setPredatorCache(await getApexPredator());
    } catch (error) {
      setPredatorError(errorMessage(error, "顶猎分数加载失败"));
    } finally {
      setPredatorLoading(false);
    }
  }, []);

  useEffect(() => {
    if (bootLoadFiredRef.current) return;
    bootLoadFiredRef.current = true;

    try {
      const rawHistory = window.localStorage.getItem(HISTORY_STORAGE_KEY);
      if (rawHistory) {
        const parsed: unknown = JSON.parse(rawHistory);
        if (Array.isArray(parsed)) {
          const history = parsed
            .filter(
              (item): item is Record<string, unknown> =>
                !!item && typeof item === "object",
            )
            .map((item) => ({
              id: String(
                item.id ??
                  `${String(item.uid ?? "unknown")}-${String(
                    item.queried_at ?? Date.now(),
                  )}`,
              ),
              queried_at: String(
                item.queried_at ?? new Date().toISOString(),
              ),
              uid: String(item.uid ?? ""),
              player_name: String(
                item.player_name ?? item.uid ?? "未知玩家",
              ),
              platform: isApexPlatform(item.platform)
                ? item.platform
                : ("PC" as const),
              level: toNullableNumber(item.level),
              rank_score: toNullableNumber(item.rank_score),
              rank_name:
                item.rank_name === null || item.rank_name === undefined
                  ? null
                  : String(item.rank_name),
              rank_div: toNullableNumber(item.rank_div),
            }))
            .filter((item) => item.uid)
            .slice(0, HISTORY_LIMIT);
          setQueryHistory(history);
        }
      }

      const lastUid = window.localStorage
        .getItem(LAST_UID_STORAGE_KEY)
        ?.trim();
      if (lastUid) {
        setLookupMode("uid");
        setPlayerQuery(lastUid);
      }
    } catch {
      setQueryHistory([]);
    }

    void getApexTranslations()
      .then(setTranslations)
      .catch(() => setTranslations({}));
    void fetchMaps();
    void fetchServerStatus();
    void fetchPredator();
  }, [fetchMaps, fetchPredator, fetchServerStatus]);

  const saveQueryHistory = (history: ApexHistoryItem[]) => {
    try {
      window.localStorage.setItem(
        HISTORY_STORAGE_KEY,
        JSON.stringify(history.slice(0, HISTORY_LIMIT)),
      );
    } catch {
      // Local history is optional; player lookup still succeeds without it.
    }
  };

  const recordQueryHistory = (data: ApexPlayerStats) => {
    const summary = data.summary;
    const uid = String(summary.uid ?? data.resolved?.uid ?? "").trim();
    if (!uid) return;

    try {
      window.localStorage.setItem(LAST_UID_STORAGE_KEY, uid);
    } catch {
      // The latest UID is only a convenience.
    }

    const item: ApexHistoryItem = {
      id: `${Date.now()}-${uid}`,
      queried_at: new Date().toISOString(),
      uid,
      player_name: summary.name || uid,
      platform: isApexPlatform(summary.platform)
        ? summary.platform
        : platform,
      level: toNullableNumber(summary.level),
      rank_score: toNullableNumber(summary.rank_score),
      rank_name: summary.rank_name || null,
      rank_div: toNullableNumber(summary.rank_div),
    };
    setQueryHistory((current) => {
      const next = [item, ...current].slice(0, HISTORY_LIMIT);
      saveQueryHistory(next);
      return next;
    });
  };

  const fetchPlayer = async (event?: FormEvent) => {
    event?.preventDefault();
    const query = playerQuery.trim();
    if (!query || playerLoading) return;

    setPlayerLoading(true);
    setPlayerError(null);
    try {
      const data =
        lookupMode === "uid"
          ? await getApexPlayer({
              uid: query,
              platform,
              save_snapshot: true,
            })
          : await getApexPlayer({
              player_name: query,
              platform,
              resolve_uid_first: true,
              save_snapshot: true,
            });
      if (!data?.summary) throw new Error("Apex 玩家数据加载失败");
      setPlayerData(data);
      recordQueryHistory(data);
    } catch (error) {
      setPlayerError(errorMessage(error, "Apex 玩家数据加载失败"));
    } finally {
      setPlayerLoading(false);
    }
  };

  const selectHistoryItem = (item: ApexHistoryItem) => {
    setLookupMode("uid");
    setPlatform(item.platform);
    setPlayerQuery(item.uid);
  };

  const changeValue = (key: string): number =>
    Number(playerData?.comparison?.changes?.[key] ?? 0);

  const rankText = playerData?.summary
    ? `${translateApexText(
        playerData.summary.rank_name,
        playerData.summary.rank_name_zh,
      )}${playerData.summary.rank_div ? ` ${playerData.summary.rank_div}` : ""}`
    : "—";

  const mapModes: Array<{ key: string } & ApexMapMode> = [
    { key: "battle_royale", ...mapCache?.data.battle_royale },
    { key: "ranked", ...mapCache?.data.ranked },
    { key: "ltm", ...mapCache?.data.ltm },
    { key: "wildcard", ...mapCache?.data.wildcard },
  ].filter((mode) => !!mode.current || !!mode.next);

  const predatorRows = Object.entries(predatorCache?.data ?? {}).map(
    ([rowPlatform, item]) => ({ platform: rowPlatform, ...item }),
  );

  return (
    <div className="p-6 space-y-5">
      <header className="flex items-start justify-between gap-5">
        <div className="flex min-w-0 items-start gap-3">
          <div className="glass-soft flex size-11 shrink-0 items-center justify-center text-lg text-blue-300">
            ◈
          </div>
          <div className="min-w-0">
            <h1 className="text-2xl font-semibold tracking-tight">
              Apex 情报
            </h1>
            <p className="mt-1 max-w-[68ch] text-sm text-white/45">
              查询玩家状态和历史变化，查看地图轮换、官方服务状态与顶猎分数线。
            </p>
          </div>
        </div>
        <div className="glass-soft flex shrink-0 gap-1 p-1">
          {VIEW_OPTIONS.map((view) => (
            <button
              key={view.value}
              type="button"
              onClick={() => setActiveView(view.value)}
              className={`inline-flex h-9 items-center gap-2 rounded-lg px-3 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400/50 ${
                activeView === view.value
                  ? "bg-blue-500/15 text-blue-100"
                  : "text-white/50 hover:bg-white/5 hover:text-white/80"
              }`}
            >
              <span aria-hidden="true">{view.icon}</span>
              {view.label}
            </button>
          ))}
        </div>
      </header>

      {activeView === "player" && (
        <div className="grid min-h-0 gap-4 xl:grid-cols-[19rem_minmax(0,1fr)]">
          <GlassCard className="h-fit">
            <h2 className="mb-3 text-[11px] font-semibold tracking-[0.15em] text-white/40">
              玩家查询
            </h2>
            <form onSubmit={fetchPlayer} className="space-y-3">
              <div className="grid grid-cols-2 gap-1 rounded-lg bg-black/20 p-1">
                {(["name", "uid"] as const).map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => setLookupMode(mode)}
                    className={`rounded-md px-3 py-1.5 text-xs transition-colors ${
                      lookupMode === mode
                        ? "bg-white/10 text-white"
                        : "text-white/45 hover:text-white/75"
                    }`}
                  >
                    {mode === "name" ? "用户名" : "UID"}
                  </button>
                ))}
              </div>
              <input
                type="text"
                value={playerQuery}
                onChange={(event) => setPlayerQuery(event.target.value)}
                placeholder={
                  lookupMode === "name"
                    ? "输入 Apex 玩家名"
                    : "输入 Apex UID"
                }
                aria-label={
                  lookupMode === "name" ? "Apex 玩家名" : "Apex UID"
                }
              />
              <select
                value={platform}
                onChange={(event) =>
                  setPlatform(event.target.value as ApexPlatform)
                }
                aria-label="游戏平台"
              >
                {PLATFORMS.map((value) => (
                  <option key={value} value={value}>
                    {PLATFORM_LABELS[value]}
                  </option>
                ))}
              </select>
              <button
                type="submit"
                disabled={playerLoading || !playerQuery.trim()}
                className="w-full rounded-lg border border-blue-400/30 bg-blue-500/20 px-4 py-2.5 text-xs font-medium text-blue-100 transition-colors hover:bg-blue-500/25 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {playerLoading ? "查询中…" : "查询玩家"}
              </button>
            </form>

            {playerError && (
              <InlineMessage tone="error">{playerError}</InlineMessage>
            )}
            {playerData?.resolved?.uid !== undefined && (
              <InlineMessage tone="info">
                已解析 UID：{String(playerData.resolved.uid)}
              </InlineMessage>
            )}
          </GlassCard>

          <div className="grid min-w-0 gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(15rem,1fr)]">
            <GlassCard className="min-h-[18rem]">
              {playerLoading ? (
                <PlayerSkeleton />
              ) : playerData?.summary ? (
                <>
                  <div className="flex items-start gap-4">
                    {playerData.summary.rank_img && (
                      <img
                        src={playerData.summary.rank_img}
                        alt="当前段位图标"
                        className="size-20 shrink-0 rounded-xl bg-white/5 object-contain ring-1 ring-white/10"
                      />
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <h2 className="truncate text-2xl font-semibold">
                          {playerData.summary.name || "未知玩家"}
                        </h2>
                        <Pill tone="info">
                          {playerData.summary.platform || platform}
                        </Pill>
                        <Pill
                          tone={
                            isTrue(playerData.summary.is_online)
                              ? "success"
                              : "neutral"
                          }
                        >
                          {isTrue(playerData.summary.is_online)
                            ? "在线"
                            : "离线"}
                        </Pill>
                      </div>
                      <div className="mt-2 font-mono text-xs text-white/40">
                        UID：{playerData.summary.uid || "—"}
                      </div>
                    </div>
                  </div>

                  <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                    <StatBlock
                      label="等级"
                      value={formatNumber(playerData.summary.level)}
                      change={changeValue("level")}
                    />
                    <StatBlock
                      label="排位分"
                      value={formatNumber(playerData.summary.rank_score)}
                      change={changeValue("rank_score")}
                    />
                    <StatBlock
                      label="段位"
                      value={rankText}
                      note={
                        playerData.comparison?.changes?.rank
                          ? "段位发生变化"
                          : undefined
                      }
                    />
                    <StatBlock
                      label="当前传奇"
                      value={translateApexText(
                        playerData.summary.selected_legend,
                        playerData.summary.selected_legend_zh,
                      )}
                    />
                  </div>

                  <div className="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                    <KeyValue
                      label="大厅状态"
                      value={translateApexText(
                        playerData.summary.lobby_state,
                        playerData.summary.lobby_state_zh,
                      )}
                    />
                    <KeyValue
                      label="当前状态"
                      value={translateApexText(
                        playerData.summary.current_state_text ||
                          playerData.summary.current_state,
                        playerData.summary.current_state_text_zh ||
                          playerData.summary.current_state_zh,
                      )}
                    />
                    <KeyValue
                      label="可加入"
                      value={
                        playerData.summary.can_join === null ||
                        playerData.summary.can_join === undefined
                          ? "—"
                          : isTrue(playerData.summary.can_join)
                            ? "是"
                            : "否"
                      }
                    />
                    <KeyValue
                      label="队伍已满"
                      value={
                        playerData.summary.party_full === null ||
                        playerData.summary.party_full === undefined
                          ? "—"
                          : isTrue(playerData.summary.party_full)
                            ? "是"
                            : "否"
                      }
                    />
                  </div>
                </>
              ) : (
                <EmptyState text="输入玩家名或 UID 后查看完整 Apex 数据。" />
              )}
            </GlassCard>

            <GlassCard className="flex min-h-[18rem] flex-col">
              <div className="mb-3 flex items-center justify-between gap-3">
                <h2 className="text-[11px] font-semibold tracking-[0.15em] text-white/40">
                  历史记录
                </h2>
                <Pill tone="neutral">{queryHistory.length}</Pill>
              </div>
              {queryHistory.length ? (
                <div className="max-h-[31rem] space-y-2 overflow-y-auto pr-1">
                  {queryHistory.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => selectHistoryItem(item)}
                      className="flex w-full items-center justify-between gap-3 rounded-lg border border-white/5 bg-white/[0.025] p-3 text-left transition-colors hover:border-blue-400/20 hover:bg-blue-400/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400/40"
                    >
                      <span className="min-w-0">
                        <span className="block truncate text-xs font-medium text-white/85">
                          {item.player_name}
                        </span>
                        <span className="block truncate font-mono text-[10px] text-white/35">
                          UID：{item.uid}
                        </span>
                        <span className="block text-[10px] text-white/30">
                          {formatDate(item.queried_at)}
                        </span>
                      </span>
                      <span className="shrink-0 text-right">
                        <span className="block font-mono text-xs font-semibold text-blue-300">
                          {formatNumber(item.rank_score)}
                        </span>
                        <span className="block text-[10px] text-white/35">
                          {translateApexText(item.rank_name)}{" "}
                          {item.rank_div || ""}
                        </span>
                      </span>
                    </button>
                  ))}
                </div>
              ) : (
                <EmptyState text="暂无历史记录" compact />
              )}
            </GlassCard>
          </div>
        </div>
      )}

      {activeView === "maps" && (
        <section className="space-y-4">
          <CacheHeader
            title="地图轮换"
            updatedAt={mapCache?.updated_at}
            loading={mapLoading}
            onRefresh={fetchMaps}
          />
          {mapError && <InlineMessage tone="error">{mapError}</InlineMessage>}
          {mapModes.length ? (
            <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
              {mapModes.map((mode) => (
                <GlassCard
                  key={mode.key}
                  padding={false}
                  className="min-h-[19rem] overflow-hidden"
                >
                  {mode.current?.asset ? (
                    <img
                      src={mode.current.asset}
                      alt=""
                      loading="lazy"
                      className="h-40 w-full object-cover"
                    />
                  ) : (
                    <div className="h-40 bg-white/[0.025]" />
                  )}
                  <div className="p-4">
                    <div className="text-[10px] font-semibold tracking-[0.15em] text-white/35">
                      {mode.name_zh || mode.name || "—"}
                    </div>
                    <h2 className="mt-2 truncate text-lg font-semibold">
                      {(mode.current?.eventName ||
                        mode.current?.eventName_zh) && (
                        <>
                          {mode.current.eventName_zh ||
                            translateApexText(mode.current.eventName)}
                          {" · "}
                        </>
                      )}
                      {mode.current?.map_zh ||
                        translateApexText(mode.current?.map)}
                    </h2>
                    <div className="mt-2">
                      <Pill tone="success">
                        {mode.current?.remainingTimer || "—"}
                      </Pill>
                    </div>
                    <div className="mt-4">
                      <KeyValue
                        label="下一张地图"
                        value={
                          mode.next?.map_zh ||
                          translateApexText(mode.next?.map)
                        }
                      />
                    </div>
                  </div>
                </GlassCard>
              ))}
            </div>
          ) : (
            <GlassCard>
              {mapLoading ? (
                <LoadingRows />
              ) : (
                <EmptyState text="暂无地图轮换数据" compact />
              )}
            </GlassCard>
          )}
        </section>
      )}

      {activeView === "servers" && (
        <section className="space-y-4">
          <CacheHeader
            title="官方服务器状态"
            updatedAt={serverCache?.updated_at}
            loading={serverLoading}
            onRefresh={fetchServerStatus}
          />
          {serverError && (
            <InlineMessage tone="error">{serverError}</InlineMessage>
          )}
          {serverCache?.data.length ? (
            <div className="grid gap-4 xl:grid-cols-2">
              {serverCache.data.map((section) => (
                <GlassCard key={section.section_key}>
                  <h2 className="mb-3 text-sm font-semibold">
                    {section.section_name_zh || section.section_name}
                  </h2>
                  <div className="grid gap-2 sm:grid-cols-2">
                    {section.rows.map((row) => (
                      <ServerStatusRow
                        key={row.key || row.name}
                        row={row}
                        translate={translateApexText}
                      />
                    ))}
                  </div>
                </GlassCard>
              ))}
            </div>
          ) : (
            <GlassCard>
              {serverLoading ? (
                <LoadingRows />
              ) : (
                <EmptyState text="暂无服务器状态数据" compact />
              )}
            </GlassCard>
          )}
        </section>
      )}

      {activeView === "predator" && (
        <section className="space-y-4">
          <CacheHeader
            title="顶尖猎杀者分数线"
            updatedAt={predatorCache?.updated_at}
            loading={predatorLoading}
            onRefresh={fetchPredator}
          />
          {predatorError && (
            <InlineMessage tone="error">{predatorError}</InlineMessage>
          )}
          <GlassCard padding={false} className="overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-white/5 text-[11px] text-white/40">
                  <th className="px-5 py-3 text-left">平台</th>
                  <th className="px-5 py-3 text-right">猎杀底分</th>
                  <th className="px-5 py-3 text-right">大师与顶猎人数</th>
                </tr>
              </thead>
              <tbody>
                {predatorRows.map((row) => (
                  <tr
                    key={row.platform}
                    className="border-b border-white/[0.03] last:border-0 hover:bg-white/[0.025]"
                  >
                    <td className="px-5 py-3 font-medium">{row.platform}</td>
                    <td className="px-5 py-3 text-right font-mono text-base font-semibold text-blue-300">
                      {formatNumber(row.val)}
                    </td>
                    <td className="px-5 py-3 text-right font-mono text-white/70">
                      {formatNumber(row.total_masters)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {!predatorRows.length && (
              <div className="p-5">
                {predatorLoading ? (
                  <LoadingRows />
                ) : (
                  <EmptyState text="暂无顶猎分数数据" compact />
                )}
              </div>
            )}
          </GlassCard>
        </section>
      )}
    </div>
  );
}

function CacheHeader({
  title,
  updatedAt,
  loading,
  onRefresh,
}: {
  title: string;
  updatedAt?: string | null;
  loading: boolean;
  onRefresh: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div>
        <h2 className="text-xl font-semibold">{title}</h2>
        <div className="mt-1 text-xs text-white/35">
          {updatedAt ? `缓存更新时间：${formatDate(updatedAt)}` : "等待数据"}
        </div>
      </div>
      <button
        type="button"
        disabled={loading}
        onClick={onRefresh}
        className="rounded-lg border border-blue-400/25 bg-blue-500/15 px-3.5 py-2 text-xs font-medium text-blue-100 transition-colors hover:bg-blue-500/20 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {loading ? "刷新中…" : "刷新"}
      </button>
    </div>
  );
}

function InlineMessage({
  tone,
  children,
}: {
  tone: "error" | "info";
  children: ReactNode;
}) {
  return (
    <div
      className={`mt-3 rounded-lg border px-3 py-2 text-xs leading-relaxed ${
        tone === "error"
          ? "border-red-400/20 bg-red-500/10 text-red-200"
          : "border-blue-400/20 bg-blue-500/10 text-blue-200"
      }`}
    >
      {children}
    </div>
  );
}

function Pill({
  tone,
  children,
}: {
  tone: "info" | "success" | "warning" | "danger" | "neutral";
  children: ReactNode;
}) {
  const toneClass = {
    info: "border-blue-400/20 bg-blue-500/10 text-blue-200",
    success: "border-emerald-400/20 bg-emerald-500/10 text-emerald-200",
    warning: "border-amber-400/20 bg-amber-500/10 text-amber-200",
    danger: "border-red-400/20 bg-red-500/10 text-red-200",
    neutral: "border-white/10 bg-white/5 text-white/55",
  }[tone];
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-semibold ${toneClass}`}
    >
      {children}
    </span>
  );
}

function StatBlock({
  label,
  value,
  change,
  note,
}: {
  label: string;
  value: string;
  change?: number;
  note?: string;
}) {
  return (
    <div className="rounded-xl border border-white/5 bg-white/[0.025] p-3">
      <div className="text-[10px] font-semibold tracking-[0.1em] text-white/35">
        {label}
      </div>
      <strong className="mt-1 block break-words text-lg font-semibold">
        {value}
      </strong>
      {!!change && (
        <span
          className={`mt-1 inline-flex rounded-full px-1.5 py-0.5 text-[10px] font-semibold ${
            change > 0
              ? "bg-emerald-500/10 text-emerald-300"
              : "bg-red-500/10 text-red-300"
          }`}
        >
          {change > 0 ? "+" : ""}
          {formatNumber(change)}
        </span>
      )}
      {note && (
        <span className="mt-1 inline-flex rounded-full bg-blue-500/10 px-1.5 py-0.5 text-[10px] font-semibold text-blue-300">
          {note}
        </span>
      )}
    </div>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-white/5 bg-black/10 px-3 py-2.5">
      <span className="text-[10px] font-semibold tracking-wide text-white/35">
        {label}
      </span>
      <b className="min-w-0 break-words text-right text-xs font-semibold text-white/80">
        {value}
      </b>
    </div>
  );
}

function ServerStatusRow({
  row,
  translate,
}: {
  row: ApexServerStatusRow;
  translate: (value?: unknown, serverChinese?: unknown) => string;
}) {
  const status = String(row.status || "").toUpperCase();
  const tone =
    status === "UP"
      ? "success"
      : status === "SLOW" || status === "OVERLOADED"
        ? "warning"
        : status === "DOWN"
          ? "danger"
          : "neutral";
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-white/5 bg-white/[0.025] p-3">
      <span className="text-xs font-medium text-white/80">
        {row.name_zh || row.name}
      </span>
      <span className="inline-flex shrink-0 items-center gap-2">
        <Pill tone={tone}>
          {translate(row.status, row.status_zh)}
        </Pill>
        <small className="w-12 text-right font-mono text-[10px] text-white/35">
          {row.response_time !== undefined && row.response_time >= 0
            ? `${row.response_time}ms`
            : "—"}
        </small>
      </span>
    </div>
  );
}

function EmptyState({
  text,
  compact = false,
}: {
  text: string;
  compact?: boolean;
}) {
  return (
    <div
      className={`flex items-center justify-center text-center text-xs text-white/35 ${
        compact ? "min-h-28" : "min-h-[14rem]"
      }`}
    >
      {text}
    </div>
  );
}

function LoadingRows() {
  return (
    <div className="space-y-2" aria-label="加载中">
      {[0, 1, 2].map((item) => (
        <div
          key={item}
          className="h-10 animate-pulse rounded-lg bg-white/5 motion-reduce:animate-none"
        />
      ))}
    </div>
  );
}

function PlayerSkeleton() {
  return (
    <div className="space-y-5" aria-label="正在加载玩家数据">
      <div className="flex items-center gap-4">
        <div className="size-20 animate-pulse rounded-xl bg-white/5 motion-reduce:animate-none" />
        <div className="flex-1 space-y-2">
          <div className="h-7 w-48 animate-pulse rounded bg-white/5 motion-reduce:animate-none" />
          <div className="h-4 w-64 animate-pulse rounded bg-white/5 motion-reduce:animate-none" />
        </div>
      </div>
      <LoadingRows />
    </div>
  );
}
