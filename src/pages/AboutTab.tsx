import { GlassCard } from "../components/GlassCard";

export function AboutTab() {
  return (
    <div className="p-6 max-w-2xl mx-auto">
      <GlassCard>
        <div className="text-2xl font-semibold mb-2">R5R 中国镜像启动器</div>
        <div className="text-white/60 mb-4">v0.4.0 · Tauri + React</div>
        <div className="text-sm text-white/70 leading-relaxed space-y-2">
          <p>
            本启动器面向中国大陆用户的 Apex（R5Reloaded）社区服。下载协议与官方
            R5Reloaded 启动器完全兼容，但所有 URL 改为可配置的镜像源。
          </p>
          <p className="text-white/50 text-xs">
            非官方项目，仅用于让大陆玩家更顺利地接入社区服务器。
          </p>
        </div>
      </GlassCard>
    </div>
  );
}
