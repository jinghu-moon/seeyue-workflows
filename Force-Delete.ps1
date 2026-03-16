# ==========================================
# 顽固文件强制删除工具
# 目标：扫描并删除 nul, con, dul 等无法用常规方法删除的文件
# ==========================================

$TargetNames = @("nul", "con", "aux", "prn", "dul") # 在这里添加你想删除的文件名

Write-Host "正在扫描顽固文件..." -ForegroundColor Cyan

# 递归扫描当前目录
$foundFiles = Get-ChildItem -Recurse -Force -ErrorAction SilentlyContinue | Where-Object { 
    $TargetNames -contains $_.Name 
}

if ($foundFiles.Count -eq 0) {
    Write-Host "未找到任何目标文件 (nul, dul, etc.)。" -ForegroundColor Green
} else {
    foreach ($file in $foundFiles) {
        $fullPath = $file.FullName
        Write-Host "发现文件: $fullPath" -ForegroundColor Yellow
        
        # 构造 UNC 路径前缀 (\\?\) 以绕过 Windows 设备名检查
        $uncPath = "\\?\$fullPath"
        
        Write-Host "正在强制删除..." -NoNewline
        
        # 调用 CMD 原生命令进行内核级删除
        cmd /c "del /f /q `"$uncPath`""
        
        # 验证是否删除成功
        if (Test-Path $fullPath) {
            Write-Host " [失败]" -ForegroundColor Red
            Write-Host "   请尝试以管理员身份运行 PowerShell。" -ForegroundColor DarkGray
        } else {
            Write-Host " [成功]" -ForegroundColor Green
        }
    }
}

Write-Host "`n完成。" -ForegroundColor Gray
Pause