# 补丁包生成与部署流程

本文档说明如何生成可被启动器“更新方式 -> 补丁包”使用的升级包。

## 产物格式

补丁包是一个 zip，zip 内必须保留安装目录结构：

```text
R5R Library/
  LIVE/
    r5apex.exe
    paks/...
```

启动器会把 zip 解压到用户配置的安装根目录，覆盖已有文件。解压完成后，启动器会按目标版本的 `checksums.json` 校验本地文件；校验通过才会把本地 `game_version` 写成新版。校验失败时会自动回退到完整“校验”更新流程。

## 准备

确认本机已安装 `uv`，然后在仓库根目录执行：

```powershell
uv --version
```

补丁包生成器由 `uv` 管理，命令入口为：

```powershell
pnpm patch:build -- --help
```

默认下载域名为 `cdn.r5r.org`，默认远端频道为 `live_game`，默认 zip 内频道目录为 `LIVE`。

默认 manifest 地址为 `https://cdn.r5r.org/launcher/live_game/checksums.json`。如果需要通过
HTTP/HTTPS 代理拉取 manifest 和下载补丁文件，可以显式指定代理：

```powershell
pnpm patch:build -- --proxy http://127.0.0.1:7890
```

也可以使用标准环境变量；未传 `--proxy` 时，Python 会自动读取它们：

```powershell
$env:HTTP_PROXY = "http://127.0.0.1:7890"
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
pnpm patch:build
```

## 首次缓存当前 manifest

每个不同 `game_version` 的 `checksums.json` 都会保存到本地：

```powershell
pnpm patch:build
```

默认保存位置：

```text
tools/patch-builder/workspace/
  manifests/
    <game_version>/
      checksums.json
```

如果已有离线的 `checksums.json`，也可以导入缓存：

```powershell
pnpm patch:build -- --to-checksums D:\path\to\checksums.json
```

## 生成补丁包

当 CDN 上的 `checksums.json` 已更新到新版本后，使用旧版本 manifest 生成差异补丁：

```powershell
pnpm patch:build -- --from-version <旧版game_version>
```

也可以直接指定旧版 manifest 文件：

```powershell
pnpm patch:build -- --from-checksums tools\patch-builder\workspace\manifests\<旧版game_version>\checksums.json
```

如果没有旧版 `checksums.json`，但手上有旧版游戏文件夹，可以用本地目录匹配。版本号由你手动指定：

```powershell
pnpm patch:build -- --from-dir D:\R5R\.31\LIVE --from-version .31
```

也可以传包含 `R5R Library\LIVE` 的上级目录：

```powershell
pnpm patch:build -- --from-dir D:\R5R\.31 --from-version .31
```

脚本会扫描旧目录中和新版 manifest 对应的文件，生成本地旧版 `checksums.json` 并保存到 `tools\patch-builder\workspace\manifests\<旧版game_version>\checksums.json`，再按差异下载新版文件。例如从 `.31` 到当前 CDN 上的 `.42` 生成补丁：

```powershell
pnpm patch:build -- --from-dir D:\R5R\.31\LIVE --from-version .31 --force
```

常用选项：

```powershell
# 指定输出目录
pnpm patch:build -- --from-version <旧版game_version> --out-dir D:\r5r-patches

# 覆盖同名本地产物
pnpm patch:build -- --from-version <旧版game_version> --force

# 包含 optional 文件，例如 HD texture pack
pnpm patch:build -- --from-version <旧版game_version> --include-optional

# 调整下载并发
pnpm patch:build -- --from-version <旧版game_version> --concurrency 8

# 仅在远端文件和 checksums.json 暂时不一致时强制打包
pnpm patch:build -- --from-version <旧版game_version> --skip-download-verify
```

`--skip-download-verify` 只跳过生成脚本下载后的 SHA-256 校验，不会修改目标 `checksums.json`。如果补丁包里包含的文件仍不满足目标 manifest，启动器应用补丁后的最终校验仍会失败并回退到完整校验。

生成后，每个补丁包会保存在单独目录：

```text
tools/patch-builder/workspace/
  patches/
    <旧版game_version>_to_<新版game_version>/
      r5r-patch-<旧版>-to-<新版>.zip
      build-report.json
      source-checksums.json
      target-checksums.json
      stage/
        R5R Library/
          LIVE/
            ...
```

运行中的临时下载和分片合并文件会统一放在 `tools\patch-builder\workspace\tmp\`，生成成功后自动清理。

`build-report.json` 中会输出可复制到 dashboard `patches[]` 的条目。

## 上传与 dashboard 配置

1. 上传 `r5r-patch-<旧版>-to-<新版>.zip` 到可公开访问的 CDN 路径。
2. 确认 URL 能被启动器直接下载，不需要鉴权或额外 Header。
3. 在 dashboard 配置接口返回的 `patches` 数组中加入：

```json
{
  "from_version": "<旧版game_version>",
  "to_version": "<新版game_version>",
  "url": "https://cdn.r5r.org/patches/r5r-patch-<旧版>-to-<新版>.zip",
  "checksum": "<补丁zip的sha256>",
  "size": 123456
}
```

`checksum` 和 `size` 会由 `build-report.json` 给出。旧版 dashboard 如暂时不方便返回这两个字段，启动器仍可下载补丁；但提供后启动器会在解压前校验 zip 本身。

4. 确认 dashboard 的 `game_version` 与新版 `checksums.json` 的 `game_version` 一致。
5. 确认 `https://cdn.r5r.org/launcher/live_game/checksums.json` 已是新版。

## 客户端使用条件

用户需要在设置中选择：

```text
更新方式：补丁包
```

启动器只支持单跳补丁：本地版本必须等于 `from_version`，远端版本必须等于 `to_version`。未找到匹配补丁、补丁下载失败、zip 结构不正确或补丁后校验失败时，启动器会回退到完整校验下载。

## 发布前检查

发布前建议至少检查：

```powershell
# 查看 zip 根目录是否正确
tar -tf tools\patch-builder\workspace\patches\<旧版>_to_<新版>\r5r-patch-<旧版>-to-<新版>.zip | Select-Object -First 20
```

期望看到的条目应以 `R5R Library/LIVE/` 开头。

发布后建议用一个旧版本安装目录测试：

1. 将启动器本地频道版本保持为旧版。
2. 设置更新方式为“补丁包”。
3. 点击“更新”。
4. 日志中应出现“尝试应用补丁包”和“补丁包应用完成”。
5. 如果日志出现“回退到完整校验”，检查 dashboard `patches[]`、zip URL、zip 目录结构和补丁后 SHA-256 校验结果。
