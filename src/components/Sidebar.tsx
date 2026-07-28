import { FormEvent, useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { useAuth } from "../hooks/useAuth";

export type TabId =
  | "home"
  | "servers"
  | "leaderboard"
  | "apex"
  | "teams"
  | "profile"
  | "launch_options"
  | "config_sync"
  | "settings"
  | "about";

interface Props {
  active: TabId;
  onChange: (tab: TabId) => void;
}

const TABS: { id: TabId; label: string; icon: string }[] = [
  { id: "home", label: "首页", icon: "▶" },
  { id: "servers", label: "服务器", icon: "☁" },
  { id: "leaderboard", label: "排行榜", icon: "♛" },
  { id: "apex", label: "Apex", icon: "◈" },
  { id: "teams", label: "组队", icon: "♟" },
  { id: "profile", label: "我的", icon: "☺" },
  { id: "launch_options", label: "启动项", icon: "⚙" },
  { id: "config_sync", label: "配置同步", icon: "⇄" },
  { id: "settings", label: "设置", icon: "✦" },
  { id: "about", label: "关于", icon: "ⓘ" },
];

export function Sidebar({ active, onChange }: Props) {
  const { appKey, user, loading, error, login, logout } = useAuth();
  const [authOpen, setAuthOpen] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const authControlRef = useRef<HTMLDivElement>(null);
  const isLoggedIn = !!appKey && !!user;

  useEffect(() => {
    if (!authOpen) return;

    const closeOnOutsideClick = (event: PointerEvent) => {
      if (!authControlRef.current?.contains(event.target as Node)) {
        setAuthOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAuthOpen(false);
    };

    document.addEventListener("pointerdown", closeOnOutsideClick);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsideClick);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [authOpen]);

  const handleLogin = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const key = keyInput.trim();
    if (!key) return;
    if (await login(key)) {
      setKeyInput("");
      setAuthOpen(false);
    }
  };

  return (
    <aside className="relative z-30 w-[88px] shrink-0 flex flex-col items-center py-4 border-r border-white/5">
      <div className="size-12 shrink-0 rounded-xl glass flex items-center justify-center text-lg font-bold mb-3">
        R5R
      </div>
      <nav className="w-full flex-1 min-h-0 overflow-y-auto flex flex-col items-center gap-1">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => {
              setAuthOpen(false);
              onChange(t.id);
            }}
            className={clsx(
              "w-16 h-14 shrink-0 rounded-xl flex flex-col items-center justify-center gap-0.5 transition-all",
              "hover:bg-white/5",
              active === t.id
                ? "bg-white/8 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]"
                : "text-white/55",
            )}
          >
            <span className="text-base leading-none">{t.icon}</span>
            <span className="text-[10px] leading-none">{t.label}</span>
          </button>
        ))}
      </nav>

      <div ref={authControlRef} className="relative shrink-0 mt-1">
        <button
          type="button"
          onClick={() => setAuthOpen((current) => !current)}
          aria-expanded={authOpen}
          className={clsx(
            "w-16 h-14 rounded-xl flex flex-col items-center justify-center gap-1 transition-all hover:bg-white/5",
            authOpen ? "bg-white/8 text-white" : "text-white/55",
          )}
          title={isLoggedIn ? `已登录：${user.player_name}` : "密钥登录"}
        >
          {isLoggedIn ? (
            <span className="size-6 rounded-full bg-blue-400/20 flex items-center justify-center text-[11px] font-bold text-blue-200">
              {user.player_name[0]?.toUpperCase()}
            </span>
          ) : (
            <svg
              viewBox="0 0 24 24"
              aria-hidden="true"
              className="size-4"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
            >
              <circle cx="8" cy="15" r="3.5" />
              <path d="m10.5 12.5 7-7m-2 2 2 2m-4-4 2 2" />
            </svg>
          )}
          <span className="max-w-14 truncate text-[10px] leading-none">
            {isLoggedIn ? user.player_name : "密钥登录"}
          </span>
        </button>

        {authOpen && (
          <div className="absolute left-[72px] bottom-0 w-72 p-4 glass text-left shadow-2xl">
            {isLoggedIn ? (
              <>
                <div className="flex items-center gap-3 mb-4">
                  <div className="size-10 rounded-full bg-blue-400/20 flex items-center justify-center text-base font-bold text-blue-200">
                    {user.player_name[0]?.toUpperCase()}
                  </div>
                  <div className="min-w-0">
                    <div className="text-sm font-medium truncate">
                      {user.player_name}
                    </div>
                    <div className="text-[11px] text-emerald-300 mt-0.5">
                      已登录
                    </div>
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setAuthOpen(false);
                      onChange("profile");
                    }}
                    className="rounded-lg bg-white/8 px-3 py-2 text-xs text-white/80 hover:bg-white/12 transition-colors"
                  >
                    查看我的
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      logout();
                      setAuthOpen(false);
                    }}
                    className="rounded-lg bg-red-500/10 px-3 py-2 text-xs text-red-200 hover:bg-red-500/15 transition-colors"
                  >
                    退出登录
                  </button>
                </div>
              </>
            ) : (
              <form onSubmit={handleLogin}>
                <div className="text-sm font-medium">使用 AppKey 登录</div>
                <div className="mt-1 mb-3 space-y-1.5 text-[11px] leading-relaxed text-white/45">
                  <p>登录后可使用组队、个人数据和社区配置上传。</p>
                  <p>
                    加入 QQ 群 732124612 后，私聊群机器人发送
                    <span className="mx-1 font-mono text-white/75">
                      「/绑定 游戏ID」
                    </span>
                    ，即可获取 AppKey。
                  </p>
                </div>
                <label
                  htmlFor="sidebar-app-key"
                  className="block text-[11px] text-white/55 mb-1.5"
                >
                  AppKey
                </label>
                <input
                  id="sidebar-app-key"
                  type="password"
                  autoComplete="current-password"
                  autoFocus
                  placeholder="请输入 AppKey"
                  value={keyInput}
                  onChange={(event) => setKeyInput(event.target.value)}
                />
                {error && (
                  <div className="text-[11px] text-red-300 mt-2">{error}</div>
                )}
                <button
                  type="submit"
                  disabled={loading || !keyInput.trim()}
                  className="w-full mt-3 rounded-lg bg-blue-500/20 border border-blue-400/30 px-3 py-2 text-xs font-medium text-blue-100 hover:bg-blue-500/25 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {loading ? "登录中…" : "登录"}
                </button>
              </form>
            )}
          </div>
        )}
      </div>

      <div className="mt-2 text-[10px] text-white/30">v{__APP_VERSION__}</div>
    </aside>
  );
}
