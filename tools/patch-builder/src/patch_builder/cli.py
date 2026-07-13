from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import shutil
import sys
import time
import urllib.parse
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from rich.console import Console
from rich.progress import (
    BarColumn,
    DownloadColumn,
    Progress,
    SpinnerColumn,
    TaskID,
    TaskProgressColumn,
    TextColumn,
    TimeRemainingColumn,
    TransferSpeedColumn,
)
from rich.table import Table

DEFAULT_CDN_DOMAIN = "cdn.r5r.org"
DEFAULT_REMOTE_CHANNEL = "live_game"
DEFAULT_INSTALL_CHANNEL = "LIVE"
DEFAULT_LANGUAGE = "schinese"
USER_AGENT = "r5r-cn-launcher-patch-builder/0.1"
CHUNK_SIZE = 1024 * 1024
TOOL_ROOT = Path(__file__).resolve().parents[2]
WORKSPACE_DIR = TOOL_ROOT / "workspace"
DEFAULT_PATCHES_DIR = WORKSPACE_DIR / "patches"
DEFAULT_MANIFESTS_DIR = WORKSPACE_DIR / "manifests"
DEFAULT_TMP_DIR = WORKSPACE_DIR / "tmp"


console = Console()


@dataclass(frozen=True)
class BuildConfig:
    from_manifest: dict[str, Any]
    to_manifest: dict[str, Any]
    cdn_domain: str
    remote_channel: str
    install_channel: str
    out_dir: Path
    concurrency: int
    languages: set[str]
    include_optional: bool
    force: bool
    all_files: bool
    skip_download_verify: bool
    proxy_url: str | None


class RemoteFileMismatchError(RuntimeError):
    def __init__(
        self,
        *,
        label: str,
        url: str,
        expected_checksum: str,
        actual_checksum: str,
        expected_size: int | None,
        actual_size: int,
    ) -> None:
        size_hint = ""
        if expected_size is not None:
            size_hint = f" expected_size={expected_size} actual_size={actual_size}"
        super().__init__(
            "远端文件与 checksums.json 不一致："
            f"{label} url={url} expected={expected_checksum} actual={actual_checksum}"
            f"{size_hint}"
        )
        self.label = label
        self.url = url
        self.expected_checksum = expected_checksum
        self.actual_checksum = actual_checksum
        self.expected_size = expected_size
        self.actual_size = actual_size


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    try:
        run(args)
    except KeyboardInterrupt:
        console.print("\n[yellow]已取消。[/yellow]")
        raise SystemExit(130) from None
    except Exception as exc:
        console.print(f"[red]失败：[/red]{exc}")
        raise SystemExit(1) from exc


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a patch zip from two R5R checksums.json manifests."
    )
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--from-checksums", type=Path, help="旧版 checksums.json 路径")
    source.add_argument("--from-dir", type=Path, help="旧版本本地游戏目录；需同时传 --from-version")
    parser.add_argument(
        "--from-version",
        help=(
            "旧版 game_version；单独使用时从本地 manifests 目录读取，"
            "配合 --from-dir 时作为旧目录版本号"
        ),
    )

    target = parser.add_mutually_exclusive_group()
    target.add_argument("--to-checksums", type=Path, help="新版 checksums.json 路径")
    target.add_argument(
        "--manifest-url",
        help="新版 checksums.json URL；默认使用 --cdn-domain 和 --remote-channel 拼出",
    )

    parser.add_argument("--cdn-domain", default=DEFAULT_CDN_DOMAIN, help="文件下载 CDN 域名")
    parser.add_argument(
        "--proxy",
        help=(
            "manifest 和文件下载使用的 HTTP/HTTPS 代理，例如 http://127.0.0.1:7890；"
            "未指定时沿用 HTTP_PROXY/HTTPS_PROXY 环境变量"
        ),
    )
    parser.add_argument("--remote-channel", default=DEFAULT_REMOTE_CHANNEL, help="CDN 频道路径")
    parser.add_argument(
        "--install-channel", default=DEFAULT_INSTALL_CHANNEL, help="zip 内安装频道目录"
    )
    parser.add_argument(
        "--out-dir", type=Path, default=DEFAULT_PATCHES_DIR, help="本地补丁输出目录"
    )
    parser.add_argument("--concurrency", type=int, default=4, help="并发下载文件数")
    parser.add_argument(
        "--languages",
        default=DEFAULT_LANGUAGE,
        help="逗号分隔的 language 白名单；空 language 永远包含",
    )
    parser.add_argument("--include-optional", action="store_true", help="包含 optional 文件")
    parser.add_argument("--all-files", action="store_true", help="不做差异过滤，打完整补丁包")
    parser.add_argument(
        "--skip-download-verify",
        action="store_true",
        help="跳过下载完成后的 SHA-256 校验；仅用于远端文件和 manifest 暂时不一致时强制打包",
    )
    parser.add_argument("--force", action="store_true", help="覆盖同版本补丁输出目录中的生成物")
    return parser.parse_args(argv)


def run(args: argparse.Namespace) -> None:
    out_dir = args.out_dir.resolve()
    manifests_dir = DEFAULT_MANIFESTS_DIR.resolve()
    manifests_dir.mkdir(parents=True, exist_ok=True)

    manifest_url = args.manifest_url or (
        f"https://{args.cdn_domain}/launcher/{args.remote_channel}/checksums.json"
    )
    proxy_url = args.proxy.strip() if args.proxy else None
    to_manifest = load_or_fetch_manifest(args.to_checksums, manifest_url, proxy_url)
    to_version = required_version(to_manifest, "新版")
    save_versioned_manifest(manifests_dir, to_manifest)

    from_manifest: dict[str, Any] | None = None
    if args.from_dir:
        if not args.from_version:
            raise ValueError(
                "--from-dir 需要同时传 --from-version，例如 --from-dir D:\\R5R31 --from-version .31"
            )
        languages = {lang.strip().lower() for lang in args.languages.split(",") if lang.strip()}
        from_manifest = build_local_manifest_from_dir(
            args.from_dir,
            args.from_version,
            to_manifest,
            args.install_channel,
            languages,
            args.include_optional,
            max(1, args.concurrency),
        )
        save_versioned_manifest(manifests_dir, from_manifest)
    elif args.from_checksums:
        from_manifest = read_json(args.from_checksums)
        save_versioned_manifest(manifests_dir, from_manifest)
    elif args.from_version:
        from_manifest = read_json(manifests_dir / safe_name(args.from_version) / "checksums.json")

    if from_manifest is None:
        console.print(
            "[green]已缓存新版 checksums.json。[/green]\n"
            "首次运行通常只会缓存当前版本；下次有新版时加上 "
            "[bold]--from-version[/bold] 或 [bold]--from-checksums[/bold] 生成补丁包。"
        )
        return

    from_version = required_version(from_manifest, "旧版")
    if from_version == to_version and not args.all_files:
        raise ValueError("旧版和新版 game_version 相同；如需完整包请加 --all-files")

    languages = {lang.strip().lower() for lang in args.languages.split(",") if lang.strip()}
    config = BuildConfig(
        from_manifest=from_manifest,
        to_manifest=to_manifest,
        cdn_domain=args.cdn_domain.strip().strip("/"),
        remote_channel=args.remote_channel.strip("/"),
        install_channel=args.install_channel,
        out_dir=out_dir,
        concurrency=max(1, args.concurrency),
        languages=languages,
        include_optional=args.include_optional,
        force=args.force,
        all_files=args.all_files,
        skip_download_verify=args.skip_download_verify,
        proxy_url=proxy_url,
    )

    build_patch(config, from_version, to_version)


def build_patch(config: BuildConfig, from_version: str, to_version: str) -> None:
    patch_name = f"{safe_name(from_version)}_to_{safe_name(to_version)}"
    patch_dir = config.out_dir / patch_name
    stage_dir = patch_dir / "stage"
    temp_dir = DEFAULT_TMP_DIR / patch_name
    zip_path = patch_dir / f"r5r-patch-{safe_name(from_version)}-to-{safe_name(to_version)}.zip"

    if patch_dir.exists() and not config.force:
        raise ValueError(f"输出目录已存在：{patch_dir}；确认覆盖请加 --force")
    shutil.rmtree(temp_dir, ignore_errors=True)
    if config.force:
        shutil.rmtree(stage_dir, ignore_errors=True)
        if zip_path.exists():
            zip_path.unlink()

    stage_dir.mkdir(parents=True, exist_ok=True)
    temp_dir.mkdir(parents=True, exist_ok=True)

    entries = diff_entries(config)
    if not entries:
        raise ValueError("没有发现需要进入补丁包的变更文件")

    total_bytes = sum(int(e.get("size", 0) or 0) for e in entries)
    print_plan(from_version, to_version, entries, total_bytes, config)

    progress = Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(),
        DownloadColumn(),
        TransferSpeedColumn(),
        TimeRemainingColumn(),
        console=console,
    )
    with progress:
        total_task = progress.add_task("下载并校验变更文件", total=max(total_bytes, 1))
        with concurrent.futures.ThreadPoolExecutor(max_workers=config.concurrency) as pool:
            futures = [
                pool.submit(
                    download_entry, entry, config, stage_dir, temp_dir, progress, total_task
                )
                for entry in entries
            ]
            for future in concurrent.futures.as_completed(futures):
                future.result()
        progress.update(total_task, completed=max(total_bytes, 1))

    shutil.rmtree(temp_dir, ignore_errors=True)
    write_zip(stage_dir, zip_path)
    zip_size = zip_path.stat().st_size
    zip_checksum = sha256_file(zip_path)

    report = {
        "from_version": from_version,
        "to_version": to_version,
        "cdn_domain": config.cdn_domain,
        "remote_channel": config.remote_channel,
        "install_channel": config.install_channel,
        "file_count": len(entries),
        "total_bytes": total_bytes,
        "zip": zip_path.name,
        "dashboard_patch_entry": {
            "from_version": from_version,
            "to_version": to_version,
            "url": f"https://{config.cdn_domain}/patches/{zip_path.name}",
            "checksum": zip_checksum,
            "size": zip_size,
        },
        "files": [normalize_manifest_path(e["path"]) for e in entries],
    }
    write_json(patch_dir / "build-report.json", report)
    write_json(patch_dir / "source-checksums.json", config.from_manifest)
    write_json(patch_dir / "target-checksums.json", config.to_manifest)

    console.print()
    console.print("[green]补丁包已生成。[/green]")
    result = Table(show_header=False)
    result.add_row("目录", str(patch_dir))
    result.add_row("ZIP", str(zip_path))
    result.add_row(
        "Dashboard patches[]", json.dumps(report["dashboard_patch_entry"], ensure_ascii=False)
    )
    console.print(result)


def load_or_fetch_manifest(path: Path | None, url: str, proxy_url: str | None) -> dict[str, Any]:
    if path:
        manifest = read_json(path)
        console.print(f"[cyan]读取新版 manifest：[/cyan]{path}")
        return manifest
    console.print(f"[cyan]拉取新版 manifest：[/cyan]{url}")
    data = fetch_bytes(url, proxy_url)
    return json.loads(data.decode("utf-8"))


def save_versioned_manifest(manifests_dir: Path, manifest: dict[str, Any]) -> None:
    version = required_version(manifest, "manifest")
    target = manifests_dir / safe_name(version) / "checksums.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    write_json(target, manifest)
    console.print(f"[dim]已保存 {version} checksums.json -> {target}[/dim]")


def build_local_manifest_from_dir(
    from_dir: Path,
    from_version: str,
    target_manifest: dict[str, Any],
    install_channel: str,
    languages: set[str],
    include_optional: bool,
    concurrency: int,
) -> dict[str, Any]:
    root = from_dir.resolve()
    if not root.exists() or not root.is_dir():
        raise ValueError(f"旧版本目录不存在或不是文件夹：{root}")

    candidates = [
        entry
        for entry in target_manifest.get("files", [])
        if entry.get("path") and should_include(entry, languages, include_optional)
    ]
    install_dir = resolve_local_install_dir(root, install_channel, candidates)
    console.print(f"[cyan]旧版本匹配目录：[/cyan]{install_dir}")

    files: list[dict[str, Any]] = []
    progress = Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(),
        TimeRemainingColumn(),
        console=console,
    )
    with progress:
        task = progress.add_task("扫描旧版本本地文件", total=max(len(candidates), 1))
        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
            futures = [
                pool.submit(hash_local_manifest_entry, install_dir, entry, progress, task)
                for entry in candidates
            ]
            for future in concurrent.futures.as_completed(futures):
                local_entry = future.result()
                if local_entry is not None:
                    files.append(local_entry)
        progress.update(task, completed=max(len(candidates), 1))

    console.print(f"[dim]旧版本本地文件匹配 {len(files)} / {len(candidates)} 个。[/dim]")
    return {
        "game_version": from_version,
        "blog_slug": target_manifest.get("blog_slug", ""),
        "languages": target_manifest.get("languages", []),
        "files": sorted(files, key=lambda e: normalize_manifest_path(e["path"]).lower()),
    }


def resolve_local_install_dir(
    root: Path, install_channel: str, entries: list[dict[str, Any]]
) -> Path:
    channel = install_channel.upper()
    candidates = [
        root,
        root / "R5R Library" / install_channel,
        root / "R5R Library" / channel,
        root / install_channel,
        root / channel,
    ]
    seen: set[Path] = set()
    unique_candidates: list[Path] = []
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved not in seen:
            seen.add(resolved)
            unique_candidates.append(resolved)

    sample = entries[: min(len(entries), 300)]
    scored: list[tuple[int, Path]] = []
    for candidate in unique_candidates:
        if not candidate.is_dir():
            continue
        hits = sum(1 for entry in sample if safe_join(candidate, entry["path"]).is_file())
        scored.append((hits, candidate))

    if not scored:
        raise ValueError(f"无法识别旧版本目录结构：{root}")

    hits, best = max(scored, key=lambda item: item[0])
    if hits == 0:
        raise ValueError(
            "旧版本目录下没有匹配到 manifest 文件；"
            f"请传 LIVE 目录或包含 R5R Library\\LIVE 的根目录：{root}"
        )
    return best


def hash_local_manifest_entry(
    install_dir: Path,
    target_entry: dict[str, Any],
    progress: Progress,
    task_id: TaskID,
) -> dict[str, Any] | None:
    try:
        path = normalize_manifest_path(target_entry["path"])
        local = safe_join(install_dir, path)
        if not local.is_file():
            return None
        entry = {
            "path": target_entry["path"],
            "size": local.stat().st_size,
            "checksum": sha256_file(local),
            "optional": bool(target_entry.get("optional", False)),
            "language": str(target_entry.get("language", "") or ""),
            "parts": [],
        }
        return entry
    finally:
        progress.update(task_id, advance=1)


def diff_entries(config: BuildConfig) -> list[dict[str, Any]]:
    old_by_path = {
        normalize_manifest_path(e.get("path", "")).lower(): e
        for e in config.from_manifest.get("files", [])
        if e.get("path")
    }
    out: list[dict[str, Any]] = []
    for entry in config.to_manifest.get("files", []):
        if not should_include(entry, config.languages, config.include_optional):
            continue
        key = normalize_manifest_path(entry.get("path", "")).lower()
        if not key:
            continue
        old = old_by_path.get(key)
        if (
            config.all_files
            or old is None
            or old.get("checksum", "").lower() != entry.get("checksum", "").lower()
        ):
            out.append(entry)
    return sorted(out, key=lambda e: normalize_manifest_path(e["path"]).lower())


def should_include(entry: dict[str, Any], languages: set[str], include_optional: bool) -> bool:
    path = normalize_manifest_path(entry.get("path", ""))
    lower = path.lower()
    if "platform/cfg/user" in lower or "platform/screenshots" in lower or "platform/logs" in lower:
        return False
    if entry.get("optional") and not include_optional:
        return False
    language = str(entry.get("language", "") or "").lower()
    return not language or language in languages


def download_entry(
    entry: dict[str, Any],
    config: BuildConfig,
    stage_dir: Path,
    temp_dir: Path,
    progress: Progress,
    total_task: TaskID,
) -> None:
    manifest_path = normalize_manifest_path(entry["path"])
    destination = safe_join(stage_dir / "R5R Library" / config.install_channel, manifest_path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    parts = entry.get("parts") or []

    if parts:
        part_paths: list[Path] = []
        entry_temp = temp_dir / hashlib.sha256(manifest_path.encode("utf-8")).hexdigest()
        entry_temp.mkdir(parents=True, exist_ok=True)
        for idx, part in enumerate(parts):
            part_path = entry_temp / f"part_{idx:06d}.bin"
            url = file_url(config, part["path"])
            expected = str(part.get("checksum", "") or "")
            size = int(part.get("size", 0) or 0)
            download_with_retry(
                url,
                part_path,
                progress,
                total_task,
                label=f"{manifest_path} part {idx}",
                expected_checksum=None if config.skip_download_verify else expected or None,
                expected_size=None if config.skip_download_verify else size if size > 0 else None,
                proxy_url=config.proxy_url,
            )
            part_paths.append(part_path)
        with destination.open("wb") as out:
            for part_path in part_paths:
                with part_path.open("rb") as src:
                    shutil.copyfileobj(src, out, length=CHUNK_SIZE)
    else:
        size = int(entry.get("size", 0) or 0)
        expected = str(entry.get("checksum", "") or "")
        download_with_retry(
            file_url(config, manifest_path),
            destination,
            progress,
            total_task,
            label=manifest_path,
            expected_checksum=None if config.skip_download_verify else expected or None,
            expected_size=None if config.skip_download_verify else size if size > 0 else None,
            proxy_url=config.proxy_url,
        )

    expected = str(entry.get("checksum", "") or "")
    if expected and not config.skip_download_verify:
        verify_sha256(destination, expected, manifest_path)


def download_with_retry(
    url: str,
    destination: Path,
    progress: Progress,
    task_id: TaskID,
    *,
    label: str,
    expected_checksum: str | None,
    expected_size: int | None,
    proxy_url: str | None,
) -> None:
    last_error: Exception | None = None
    for attempt in range(1, 4):
        request_url = add_cache_buster(url, attempt) if attempt > 1 else url
        bytes_written = 0
        try:
            bytes_written = download_file(request_url, destination, progress, task_id, proxy_url)
            if expected_checksum:
                verify_downloaded_file(
                    destination,
                    label=label,
                    url=request_url,
                    expected_checksum=expected_checksum,
                    expected_size=expected_size,
                )
            return
        except Exception as exc:
            last_error = exc
            if destination.exists():
                destination.unlink()
            if bytes_written > 0:
                progress.update(task_id, advance=-bytes_written)
            time.sleep(min(2**attempt, 8))
    if isinstance(last_error, RemoteFileMismatchError):
        raise last_error
    raise RuntimeError(f"下载失败 {url}: {last_error}") from last_error


def download_file(
    url: str,
    destination: Path,
    progress: Progress,
    task_id: TaskID,
    proxy_url: str | None,
) -> int:
    tmp = destination.with_suffix(destination.suffix + ".download")
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    bytes_written = 0
    with open_url(request, proxy_url) as response, tmp.open("wb") as out:
        status = getattr(response, "status", 200)
        if status < 200 or status >= 300:
            raise RuntimeError(f"HTTP {status}")
        while True:
            chunk = response.read(CHUNK_SIZE)
            if not chunk:
                break
            out.write(chunk)
            bytes_written += len(chunk)
            progress.update(task_id, advance=len(chunk))
    tmp.replace(destination)
    return bytes_written


def verify_downloaded_file(
    path: Path,
    *,
    label: str,
    url: str,
    expected_checksum: str,
    expected_size: int | None,
) -> None:
    actual_size = path.stat().st_size
    actual_checksum = sha256_file(path)
    if actual_checksum.lower() != expected_checksum.lower():
        raise RemoteFileMismatchError(
            label=label,
            url=url,
            expected_checksum=expected_checksum,
            actual_checksum=actual_checksum,
            expected_size=expected_size,
            actual_size=actual_size,
        )


def add_cache_buster(url: str, attempt: int) -> str:
    split = urllib.parse.urlsplit(url)
    query = f"{split.query}&" if split.query else ""
    query += f"_r5r_patch_retry={attempt}-{int(time.time())}"
    return urllib.parse.urlunsplit((split.scheme, split.netloc, split.path, query, split.fragment))


def fetch_bytes(url: str, proxy_url: str | None) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with open_url(request, proxy_url) as response:
        status = getattr(response, "status", 200)
        if status < 200 or status >= 300:
            raise RuntimeError(f"{url} HTTP {status}")
        return response.read()


def open_url(request: urllib.request.Request, proxy_url: str | None) -> Any:
    if not proxy_url:
        return urllib.request.urlopen(request, timeout=30)
    proxy_handler = urllib.request.ProxyHandler({"http": proxy_url, "https": proxy_url})
    return urllib.request.build_opener(proxy_handler).open(request, timeout=30)


def file_url(config: BuildConfig, path_or_url: str) -> str:
    raw = path_or_url.replace("\\", "/")
    parsed = urllib.parse.urlparse(raw)
    if parsed.scheme and parsed.netloc:
        parsed = parsed._replace(scheme="https", netloc=config.cdn_domain)
        return urllib.parse.urlunparse(parsed)
    quoted = urllib.parse.quote(raw.lstrip("/"), safe="/")
    return f"https://{config.cdn_domain}/launcher/{config.remote_channel}/{quoted}"


def write_zip(stage_dir: Path, zip_path: Path) -> None:
    files = sorted(p for p in stage_dir.rglob("*") if p.is_file())
    with zipfile.ZipFile(
        zip_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6, allowZip64=True
    ) as zf:
        for path in files:
            zf.write(path, path.relative_to(stage_dir).as_posix())


def verify_sha256(path: Path, expected: str, label: str) -> None:
    actual = sha256_file(path)
    if actual.lower() != expected.lower():
        raise RuntimeError(f"SHA-256 不一致：{label} expected={expected} actual={actual}")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(CHUNK_SIZE), b""):
            h.update(chunk)
    return h.hexdigest()


def normalize_manifest_path(path: str) -> str:
    return str(path or "").replace("\\", "/").strip("/")


def safe_join(root: Path, manifest_path: str) -> Path:
    parts = PurePosixPath(normalize_manifest_path(manifest_path)).parts
    if not parts or any(part in ("", ".", "..") for part in parts):
        raise ValueError(f"manifest path 不安全：{manifest_path}")
    return root.joinpath(*parts)


def required_version(manifest: dict[str, Any], label: str) -> str:
    version = str(manifest.get("game_version", "") or "").strip()
    if not version:
        raise ValueError(f"{label}缺少 game_version")
    return version


def safe_name(value: str) -> str:
    return "".join(c if c.isalnum() or c in "._-" else "_" for c in value.strip()) or "unknown"


def read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=2)
        f.write("\n")


def print_plan(
    from_version: str,
    to_version: str,
    entries: list[dict[str, Any]],
    total_bytes: int,
    config: BuildConfig,
) -> None:
    table = Table(title="补丁包生成计划")
    table.add_column("项目")
    table.add_column("值")
    table.add_row("版本", f"{from_version} -> {to_version}")
    table.add_row("文件数", str(len(entries)))
    table.add_row("体积", f"{total_bytes:,} bytes")
    table.add_row("下载源", f"https://{config.cdn_domain}/launcher/{config.remote_channel}/")
    table.add_row("代理", "显式代理" if config.proxy_url else "环境变量或系统默认")
    table.add_row("zip 根目录", f"R5R Library/{config.install_channel}/")
    table.add_row("optional", "包含" if config.include_optional else "跳过")
    table.add_row("下载后校验", "跳过" if config.skip_download_verify else "启用")
    console.print(table)


if __name__ == "__main__":
    main(sys.argv[1:])
