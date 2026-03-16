import os
import asyncio
import aiohttp
import aiofiles
import re
import json
from urllib.parse import urlparse

# ── 配置 ──────────────────────────────────────────────────────────────────────
REPO_OWNER   = "modelcontextprotocol"
REPO_NAME    = "modelcontextprotocol"
BRANCH       = "main"
DOCS_ROOT    = "docs"          # 仓库内的文档目录
OUTPUT_ROOT  = "mcp_docs"      # 本地输出目录
CONCURRENCY  = 12
SKIP_EXISTING = True

# 只下载文档内容目录（排除配置文件、well-known 等）
ALLOWED_EXTENSIONS = {".mdx", ".md"}
SKIP_PATHS = {
    "docs/.well-known",
    "docs/docs.json",
}

GH_API_BASE  = "https://api.github.com"
GH_RAW_BASE  = "https://raw.githubusercontent.com"

HEADERS = {
    "User-Agent": "mcp-docs-crawler/2.0",
    "Accept":     "application/vnd.github.v3+json",
}


# ── GitHub API：递归获取文件树 ────────────────────────────────────────────────
async def fetch_file_tree(session: aiohttp.ClientSession) -> list[dict]:
    """
    通过 Git Trees API（recursive=1）一次性获取整个仓库文件树。
    返回所有 docs/ 下的 blob（文件）条目列表。
    """
    url = f"{GH_API_BASE}/repos/{REPO_OWNER}/{REPO_NAME}/git/trees/{BRANCH}?recursive=1"
    async with session.get(url, headers=HEADERS) as resp:
        resp.raise_for_status()
        data = await resp.json()

    blobs = []
    for item in data.get("tree", []):
        if item["type"] != "blob":
            continue
        path = item["path"]
        if not path.startswith(DOCS_ROOT + "/"):
            continue
        if any(path.startswith(s) for s in SKIP_PATHS):
            continue
        _, ext = os.path.splitext(path)
        if ext.lower() not in ALLOWED_EXTENSIONS:
            continue
        blobs.append(item)

    return blobs


# ── URL → 本地路径映射 ────────────────────────────────────────────────────────
def repo_path_to_local(repo_path: str) -> str:
    """
    将仓库路径映射到本地输出路径。
    docs/docs/getting-started/intro.mdx → mcp_docs/docs/getting-started/intro.md
    docs/clients.mdx                    → mcp_docs/clients.md
    """
    # 去掉 docs/ 前缀
    rel = repo_path[len(DOCS_ROOT) + 1:]   # e.g. "docs/getting-started/intro.mdx"
    # 统一扩展名为 .md
    base, _ = os.path.splitext(rel)
    local_rel = base + ".md"
    local_path = os.path.join(OUTPUT_ROOT, local_rel)
    os.makedirs(os.path.dirname(local_path) or ".", exist_ok=True)
    return local_path


# ── MDX → Markdown 轻量清理 ───────────────────────────────────────────────────
def mdx_to_md(content: str, repo_path: str) -> str:
    """
    MDX 文件通常含有 JSX import/component 语法。
    做最小化清理：去除 import 行、<Component> 标签，保留正文 Markdown。
    """
    lines = content.splitlines()
    cleaned = []
    in_frontmatter = False
    frontmatter_done = False
    fm_count = 0

    for line in lines:
        stripped = line.strip()

        # 保留 YAML frontmatter 块
        if stripped == "---" and not frontmatter_done:
            fm_count += 1
            if fm_count <= 2:
                cleaned.append(line)
                if fm_count == 2:
                    frontmatter_done = True
                continue

        # 跳过 JSX import 行
        if re.match(r'^import\s+', stripped):
            continue

        # 移除自闭合 JSX 标签（保留内容）
        # e.g. <Tip>...</Tip> → 保留内容，去掉标签
        line = re.sub(r'<[A-Z][A-Za-z0-9]*(?:\s[^>]*)?\/>', '', line)
        line = re.sub(r'<\/[A-Z][A-Za-z0-9]*>', '', line)
        line = re.sub(r'<[A-Z][A-Za-z0-9]*(?:\s[^>]*)?>', '', line)

        cleaned.append(line)

    result = "\n".join(cleaned)
    # 压缩多余空行
    result = re.sub(r'\n{3,}', '\n\n', result).strip()
    return result


# ── 爬虫主体 ─────────────────────────────────────────────────────────────────
class MCPDocsCrawler:
    def __init__(self):
        self.semaphore = asyncio.Semaphore(CONCURRENCY)
        self.session: aiohttp.ClientSession | None = None
        self.success_count = 0
        self.skip_count    = 0
        self.fail_count    = 0

    async def download_blob(self, item: dict):
        repo_path  = item["path"]
        local_path = repo_path_to_local(repo_path)

        if SKIP_EXISTING and os.path.exists(local_path):
            self.skip_count += 1
            print(f"⏭  已存在，跳过: {repo_path}")
            return

        raw_url = f"{GH_RAW_BASE}/{REPO_OWNER}/{REPO_NAME}/{BRANCH}/{repo_path}"

        async with self.semaphore:
            try:
                async with self.session.get(
                    raw_url,
                    headers={"User-Agent": HEADERS["User-Agent"]},
                    timeout=aiohttp.ClientTimeout(total=30),
                ) as resp:
                    if resp.status != 200:
                        print(f"✗  HTTP {resp.status}: {repo_path}")
                        self.fail_count += 1
                        return
                    content = await resp.text(encoding="utf-8", errors="replace")
            except Exception as e:
                print(f"✗  请求失败 {repo_path}: {e}")
                self.fail_count += 1
                return

        # MDX → MD 清理
        md_content = mdx_to_md(content, repo_path)

        async with aiofiles.open(local_path, "w", encoding="utf-8") as f:
            await f.write(md_content + "\n")

        self.success_count += 1
        print(f"✓  {repo_path:<70} → {local_path}")

    # ── 目录文件生成 ──────────────────────────────────────────────────────────
    def generate_toc(self):
        entries = []
        for root, dirs, files in os.walk(OUTPUT_ROOT):
            dirs.sort()
            for fname in sorted(files):
                if not fname.endswith(".md") or fname == "_index.md":
                    continue
                full  = os.path.join(root, fname)
                rel   = os.path.relpath(full, OUTPUT_ROOT).replace("\\", "/")
                try:
                    with open(full, "r", encoding="utf-8") as f:
                        head = f.read(4096)
                    m = re.search(r'^title:\s*["\']?(.+?)["\']?\s*$', head, re.MULTILINE)
                    title = m.group(1).strip() if m else rel
                except Exception:
                    title = rel
                depth  = rel.count("/")
                indent = "  " * depth
                entries.append(f"{indent}- [{title}](./{rel})")

        toc_path = os.path.join(OUTPUT_ROOT, "_index.md")
        with open(toc_path, "w", encoding="utf-8") as f:
            f.write("# MCP 官方文档目录\n\n")
            f.write("> 自动生成，来源：https://github.com/modelcontextprotocol/modelcontextprotocol\n\n")
            f.write("\n".join(entries) + "\n")
        print(f"\n📋 目录文件已生成: {toc_path}")

    # ── 入口 ──────────────────────────────────────────────────────────────────
    async def run(self):
        os.makedirs(OUTPUT_ROOT, exist_ok=True)
        connector = aiohttp.TCPConnector(limit=CONCURRENCY, ssl=True)
        async with aiohttp.ClientSession(connector=connector) as session:
            self.session = session

            print("🔍 正在获取仓库文件树...")
            blobs = await fetch_file_tree(session)
            print(f"   共发现 {len(blobs)} 个文档文件\n")

            tasks = [self.download_blob(b) for b in blobs]
            await asyncio.gather(*tasks)

        self.generate_toc()
        print(f"\n{'='*60}")
        print(f"✅ 抓取完成！")
        print(f"   成功: {self.success_count} 页")
        print(f"   跳过: {self.skip_count} 页（已存在）")
        print(f"   失败: {self.fail_count} 页")
        print(f"   输出目录: {os.path.abspath(OUTPUT_ROOT)}")
        print(f"{'='*60}")


if __name__ == "__main__":
    print("🔥 MCP 官方文档抓取开始（GitHub Raw 模式）...")
    print(f"   仓库: {REPO_OWNER}/{REPO_NAME} @ {BRANCH}")
    print(f"   文档目录: {DOCS_ROOT}/")
    print(f"   输出目录: {OUTPUT_ROOT}/")
    print(f"   并发数: {CONCURRENCY}")
    print(f"   断点续爬: {'开启' if SKIP_EXISTING else '关闭'}")
    print()
    asyncio.run(MCPDocsCrawler().run())
