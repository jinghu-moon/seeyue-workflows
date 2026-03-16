"""convert_mcp_docs.py

将本地 MCP 官方文档源码（.mdx）批量转换为纯 Markdown（.md），
保留目录结构，输出到 mcp_docs/。

用法：
    python convert_mcp_docs.py
"""

import os
import re
import shutil
from pathlib import Path

# ── 配置 ──────────────────────────────────────────────────────────────────────
SOURCE_ROOT = Path(r"refer/mcp-source/modelcontextprotocol-main/docs")
OUTPUT_ROOT = Path("mcp_docs")

# 只转换 .mdx / .md 文本文件
TEXT_EXTENSIONS = {".mdx", ".md"}

# 跳过的路径（相对于 SOURCE_ROOT）
SKIP_PATHS = {
    ".well-known",
    "docs.json",
    "footer.js",
    "favicon.svg",
}

# 图片/二进制文件：直接复制到 mcp_docs/images/
IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp"}


# ── JSX / MDX 清理 ────────────────────────────────────────────────────────────
# 已知 Mintlify 组件列表（用于精确匹配，防止误删 HTML）
MINTLIFY_COMPONENTS = {
    "Frame", "Card", "CardGroup", "Tip", "Note", "Warning", "Info",
    "Accordion", "AccordionGroup", "Tabs", "Tab", "Steps", "Step",
    "CodeGroup", "ResponseField", "ParamField", "Expandable",
    "Icon", "Tooltip", "Check", "Update", "Badge",
}


def strip_mdx(content: str) -> str:
    """将 MDX 内容转换为纯 Markdown。"""
    lines = content.splitlines()
    out = []
    fm_dashes = 0          # frontmatter --- 计数
    in_frontmatter = True  # 文件开头可能有 frontmatter

    for line in lines:
        stripped = line.strip()

        # ── frontmatter 边界跟踪 ────────────────────────────────────────────
        if in_frontmatter and stripped == "---":
            fm_dashes += 1
            out.append(line)
            if fm_dashes == 2:
                in_frontmatter = False
            continue

        # 首行非 --- 说明无 frontmatter
        if in_frontmatter and fm_dashes == 0 and stripped != "---":
            in_frontmatter = False

        # ── import / export 语句 ────────────────────────────────────────────
        if re.match(r'^\s*(import|export)\s+', line):
            continue

        # ── Mintlify 组件标签 ───────────────────────────────────────────────
        # 构建组件名匹配模式
        comp_pat = "|".join(re.escape(c) for c in MINTLIFY_COMPONENTS)

        # 自闭合：<Tip ... />
        line = re.sub(rf'<({comp_pat})(\s[^>]*)?\/>', '', line)
        # 闭合标签：</Card>
        line = re.sub(rf'</({comp_pat})>', '', line)
        # 开标签（保留内容，去掉标签行）：<Card title="..." href="...">
        # 若整行只有标签，跳过；否则只去掉标签
        open_tag_re = re.compile(rf'^\s*<({comp_pat})(\s[^>]*)?\s*>\s*$')
        if open_tag_re.match(line):
            continue
        line = re.sub(rf'<({comp_pat})(\s[^>]*)?>', '', line)

        # ── 通用大写开头 JSX 残余 ───────────────────────────────────────────
        # 捕获未列入上表但仍以大写开头的 JSX 标签
        line = re.sub(r'<[A-Z][A-Za-z0-9]*(\s[^>]*)?\/>', '', line)
        line = re.sub(r'</[A-Z][A-Za-z0-9]*>', '', line)
        open_generic = re.compile(r'^\s*<[A-Z][A-Za-z0-9]*(\s[^>]*)?\s*>\s*$')
        if open_generic.match(line):
            continue
        line = re.sub(r'<[A-Z][A-Za-z0-9]*(\s[^>]*)?>', '', line)

        # ── Card / href 属性提取（将 Card 转为 Markdown 链接）──────────────
        # 已在上方被展开为空行，此处不需要额外处理

        out.append(line)

    result = "\n".join(out)
    # 压缩 3+ 连续空行为 2 行
    result = re.sub(r'\n{3,}', '\n\n', result)
    return result.strip()


# ── 路径映射 ──────────────────────────────────────────────────────────────────
def source_to_output(src_path: Path) -> Path:
    """将源文件路径映射到输出路径（.mdx → .md，其余保持扩展名）。"""
    rel = src_path.relative_to(SOURCE_ROOT)
    parts = list(rel.parts)

    # 统一扩展名
    stem = rel.stem
    suffix = rel.suffix.lower()
    if suffix == ".mdx":
        suffix = ".md"

    new_rel = Path(*parts[:-1]) / (stem + suffix) if len(parts) > 1 else Path(stem + suffix)
    return OUTPUT_ROOT / new_rel


# ── 主转换逻辑 ────────────────────────────────────────────────────────────────
def convert_all():
    converted = 0
    copied    = 0
    skipped   = 0
    errors    = 0

    for src in sorted(SOURCE_ROOT.rglob("*")):
        if not src.is_file():
            continue

        rel = src.relative_to(SOURCE_ROOT)
        rel_str = rel.as_posix()

        # 跳过规则
        if any(rel_str == s or rel_str.startswith(s + "/") for s in SKIP_PATHS):
            skipped += 1
            continue

        ext = src.suffix.lower()
        dst = source_to_output(src)
        dst.parent.mkdir(parents=True, exist_ok=True)

        if ext in TEXT_EXTENSIONS:
            try:
                raw = src.read_text(encoding="utf-8", errors="replace")
                md  = strip_mdx(raw)
                dst.write_text(md + "\n", encoding="utf-8")
                print(f"✓  {rel_str}")
                converted += 1
            except Exception as e:
                print(f"✗  {rel_str}: {e}")
                errors += 1

        elif ext in IMAGE_EXTENSIONS:
            shutil.copy2(src, dst)
            copied += 1

        else:
            # 其他文件类型跳过
            skipped += 1

    return converted, copied, skipped, errors


# ── 目录索引生成 ──────────────────────────────────────────────────────────────
def generate_toc():
    entries = []
    for md_file in sorted(OUTPUT_ROOT.rglob("*.md")):
        if md_file.name == "_index.md":
            continue
        rel = md_file.relative_to(OUTPUT_ROOT).as_posix()
        try:
            head = md_file.read_text(encoding="utf-8")[:2048]
            m = re.search(r'^title:\s*["\']?(.+?)["\']?\s*$', head, re.MULTILINE)
            title = m.group(1).strip() if m else rel
        except Exception:
            title = rel
        depth  = rel.count("/")
        indent = "  " * depth
        entries.append(f"{indent}- [{title}](./{rel})")

    toc = OUTPUT_ROOT / "_index.md"
    toc.write_text(
        "# MCP 官方文档目录\n\n"
        "> 自动生成，来源：refer/mcp-source/modelcontextprotocol-main/docs\n\n"
        + "\n".join(entries) + "\n",
        encoding="utf-8",
    )
    print(f"\n📋 目录文件已生成: {toc}")


# ── 入口 ──────────────────────────────────────────────────────────────────────
if __name__ == "__main__":
    print("🔥 MCP 文档本地转换开始...")
    print(f"   源目录: {SOURCE_ROOT}")
    print(f"   输出目录: {OUTPUT_ROOT}")
    print()

    converted, copied, skipped, errors = convert_all()
    generate_toc()

    print(f"\n{'='*60}")
    print(f"✅ 转换完成！")
    print(f"   转换 .mdx→.md: {converted} 个")
    print(f"   复制图片:       {copied} 个")
    print(f"   跳过:           {skipped} 个")
    print(f"   错误:           {errors} 个")
    print(f"   输出目录: {OUTPUT_ROOT.resolve()}")
    print(f"{'='*60}")
