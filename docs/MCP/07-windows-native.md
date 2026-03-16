# Windows 原生优化层

> 来源：`docs/hooks-architecture-design.md` §1.2，`docs/seeyue-workflows-hooks-architecture-windows.md`
> 参考：`refer/MCP-DEMO/V5-DESIGN.md` §三，`refer/MCP-DEMO/encoding_layer.rs`
> 参考：`docs/seeyue-workflows-mcp-integration-windows.md` §2.4

---

## 1. 设计原则

来源：`docs/hooks-architecture-design.md` §1.2

**100% 专注 Windows 平台，充分利用 Windows 原生特性，不追求跨平台兼容。**

| Windows 特性 | 用途 | 性能/优势 |
|-------------|------|-----------|
| NTFS ACL | 文件保护 | 操作系统级强制访问控制 |
| VSS（Volume Shadow Copy）| 快照 | 零空间开销，写时复制 |
| Windows 注册表 | 状态存储 | 比文件 I/O 快 100x |
| IOCP（I/O Completion Ports）| 异步 I/O | Windows 原生高性能异步 |
| Windows Job Objects | 资源限制 | 进程级内存/CPU 限制 |
| PowerShell AST | 命令解析 | 精确解析 PS1 脚本 |
| Windows Credential Manager | 密钥存储 | 系统级加密 |
| Windows 事件日志 | 审计 | 企业 SIEM 集成 |

---

## 2. 路径规范化

来源：`refer/MCP-DEMO/V5-DESIGN.md` §三.3.1

```
Agent 输入："src/auth/jwt.rs"（正斜杠）
     │
     ▼  replace('/', '\\')
"src\\auth\\jwt.rs"
     │
     ▼  join(workspace)
"D:\\Projects\\seeyue-workflows\\src\\auth\\jwt.rs"
     │
     ▼  路径逃逸检查（防止 ../../../ 穿越）
     │
     ▼  超过 260 字符时自动加 \\?\\ 前缀（长路径支持）
"\\\\?\\D:\\Projects\\seeyue-workflows\\src\\auth\\jwt.rs"
```

实现位置：`src/platform/path.rs`

```rust
// platform/path.rs（来源：refer/MCP-DEMO/V5-DESIGN.md §三）
pub fn resolve(workspace: &Path, input: &str) -> Result<PathBuf, ToolError> {
    let normalized = input.replace('/', "\\\\");
    let candidate = workspace.join(&normalized);
    let canonical = candidate.canonicalize()
        .map_err(|_| ToolError::PathNotFound(input.to_string()))?;
    // 逃逸检查
    if !canonical.starts_with(workspace) {
        return Err(ToolError::PathEscape { path: input.to_string() });
    }
    // 长路径（> 260 字符）
    if canonical.to_string_lossy().len() > 260 {
        let unc = format!("\\\\\\\\?\\\\{}", canonical.display());
        return Ok(PathBuf::from(unc));
    }
    Ok(canonical)
}
```

---

## 3. 编码检测

来源：`docs/seeyue-workflows-mcp-integration-windows.md` §2.4.1，`refer/MCP-DEMO/encoding_layer.rs`

```rust
// encoding_layer.rs（来源：refer/MCP-DEMO）
pub fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    // 1. BOM 检测（最快）
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { return UTF_8; }
    if bytes.starts_with(&[0xFF, 0xFE])       { return UTF_16LE; }
    if bytes.starts_with(&[0xFE, 0xFF])       { return UTF_16BE; }

    // 2. chardetng（Firefox 同款）
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let encoding = detector.guess(None, true);

    // 3. 回退 UTF-8
    encoding
}
```

**Windows 专项编码支持**：
- GBK：简体中文 Windows 默认编码（系统 Codepage 936）
- Shift-JIS：日文 Windows（Codepage 932）
- UTF-16LE：Windows 原生 Unicode 文本文件（Notepad 默认）
- CRLF：Windows 默认行尾（`\\r\\n`），写入时保留

**GetACP() 集成**（来源：`refer/MCP-DEMO/V5-DESIGN.md` §参考资料，`windows-sys` v0.52）：

```rust
// 获取系统 Codepage 作为 chardetng 后备
#[cfg(windows)]
fn system_codepage() -> Option<&'static Encoding> {
    use windows_sys::Win32::Globalization::GetACP;
    match unsafe { GetACP() } {
        936 => Some(GBK),
        932 => Some(SHIFT_JIS),
        _   => None,
    }
}
```

---

## 4. CRLF 处理

来源：`refer/MCP-DEMO/V5-DESIGN.md` §三.3.2，Claude Code Issue #13456

```rust
pub struct EncodingInfo {
    pub encoding: &'static Encoding,
    pub has_bom:  bool,
    pub line_ending: LineEnding,  // CRLF | LF | Mixed
}

pub enum LineEnding { CRLF, LF, Mixed }

// 写入时保留原文件行尾风格
pub fn safe_encode(content: &str, info: &EncodingInfo) -> Vec<u8> {
    let normalized = match info.line_ending {
        LineEnding::CRLF  => content.replace('\\n', "\\r\\n"),
        LineEnding::LF    => content.replace("\\r\\n", "\\n"),
        LineEnding::Mixed => content.to_string(),
    };
    // encoding_rs 编码
    let (encoded, _, _) = info.encoding.encode(&normalized);
    if info.has_bom { add_bom(info.encoding, encoded.into_owned()) }
    else { encoded.into_owned() }
}
```

---

## 5. NTFS ACL 保护

来源：`docs/seeyue-workflows-hooks-architecture-windows.md` §1.1.2，`docs/hooks-architecture-design.md` §1.2

对 `critical_policy_file` 和 `security_boundary` 类文件设置 NTFS ACL，防止未授权写入：

```powershell
# scripts/windows/apply-ntfs-protection.ps1
# 来源：docs/hooks-architecture-design.md §1.1.2
param([string]$FilePath)

$acl = Get-Acl $FilePath
$acl.SetAccessRuleProtection($true, $false)  # 断开继承

# 移除 Users 写权限
$deny = New-Object System.Security.AccessControl.FileSystemAccessRule(
    "Users", "Write,Delete", "Deny"
)
$acl.AddAccessRule($deny)
Set-Acl -Path $FilePath -AclObject $acl
```

**触发时机**：`sy_pretool_write` 工具验证阶段，检测到 `system_file` / `critical_policy_file` 类时调用。

---

## 6. 终端 ANSI 检测

来源：`refer/MCP-DEMO/V5-DESIGN.md` §三.3.4，`crossterm` v0.28

```rust
// platform/terminal.rs
pub fn supports_ansi() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            GetConsoleMode, GetStdHandle, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
            STD_OUTPUT_HANDLE,
        };
        let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        let mut mode = 0u32;
        if unsafe { GetConsoleMode(handle, &mut mode) } != 0 {
            return mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0;
        }
    }
    std::env::var("TERM").is_ok()  // fallback
}
```

Diff 输出：支持 ANSI 时使用彩色渲染，不支持时退回纯文本。

---

## 7. Windows 异步 I/O（tokio + IOCP）

来源：`refer/MCP-DEMO/Cargo.toml`，`refer/MCP-DEMO/V5-DESIGN.md` §一

```toml
# tokio 的 Windows 后端使用 IOCP（I/O Completion Ports）
tokio = { version = "1", features = ["full"] }
```

tokio 在 Windows 上自动使用 IOCP，无需额外配置。对比 epoll（Linux）：
- IOCP 是内核级异步，无轮询开销
- 高并发文件 I/O 场景下性能更优
- MCP stdio 传输层也受益于此

---

## 8. Windows Defender 启动加速

来源：`refer/MCP-DEMO/V5-DESIGN.md` §一

单文件 Rust 二进制的 Windows Defender 特性：
- 首次运行：Defender 扫描二进制（约 50-200ms）
- 后续运行：扫描结果缓存，启动时间 < 30ms
- 对比 Node.js：每次启动扫描 `node_modules/`（数千文件），无法缓存

构建优化（`Cargo.toml`）：

```toml
[profile.release]
strip = true      # 移除调试符号，减小二进制体积
lto = true        # 链接时优化
opt-level = "z"   # 最小体积优化
```
