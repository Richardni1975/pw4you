# pw4you - Graphical Installer
# ================================
# Pure PowerShell + .NET Windows Forms GUI installer.
# No external tools required — built into Windows.
#
# Double-click to run, or:
#   powershell -ExecutionPolicy Bypass -File gui-installer.ps1

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName Microsoft.VisualBasic

# ── Configuration ──────────────────────────────────
$AppName    = "pw4you"
$AppVersion = "0.1.0"
$Publisher  = "pw4you"
$AppExe     = "pw4you.exe"
$InstallDir = "$env:LOCALAPPDATA\pw4you"
$SourceExe  = Join-Path $PSScriptRoot "..\target\release\$AppExe"

# Colors
$bgColor   = [System.Drawing.Color]::FromArgb(245, 247, 250)
$accent    = [System.Drawing.Color]::FromArgb(30, 80, 180)
$textColor = [System.Drawing.Color]::FromArgb(30, 30, 40)
$greenOk   = [System.Drawing.Color]::FromArgb(40, 160, 80)

# ── Helper Functions ───────────────────────────────
function Write-InstallLog { param($msg) Add-Content "$env:TEMP\pw4you-install.log" "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $msg" }

function Install-App {
    param($destDir, $createDesktop, $createStartMenu, $assocFiles, $autoStart)

    Write-InstallLog "Installing to: $destDir"

    # 1. Create directories
    New-Item -ItemType Directory -Force -Path $destDir | Out-Null
    New-Item -ItemType Directory -Force -Path "$destDir\data" | Out-Null

    # 2. Copy executable
    if (-not (Test-Path $SourceExe)) {
        throw "Cannot find $SourceExe. Please build the project first."
    }
    Copy-Item $SourceExe $destDir -Force
    Write-InstallLog "Copied executable"

    $installedExe = Join-Path $destDir $AppExe

    # 3. Register uninstall in Control Panel
    $regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\pw4you"
    New-Item -Path $regPath -Force | Out-Null
    Set-ItemProperty -Path $regPath -Name "DisplayName"     -Value "pw4you - 文件夹保险箱"
    Set-ItemProperty -Path $regPath -Name "DisplayVersion"  -Value $AppVersion
    Set-ItemProperty -Path $regPath -Name "Publisher"       -Value $Publisher
    Set-ItemProperty -Path $regPath -Name "DisplayIcon"     -Value $installedExe
    Set-ItemProperty -Path $regPath -Name "UninstallString" -Value "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$destDir\uninstall.ps1`""
    Set-ItemProperty -Path $regPath -Name "InstallLocation" -Value $destDir
    Set-ItemProperty -Path $regPath -Name "NoModify"        -Value 0
    Set-ItemProperty -Path $regPath -Name "NoRepair"        -Value 1
    Write-InstallLog "Registered in Add/Remove Programs"

    # 4. Create uninstaller script
    @"
# pw4you Uninstaller
`$AppName = "$AppName"
`$InstallDir = "$destDir"
`$installedExe = "$installedExe"

# Kill running instances
Get-Process pw4you -ErrorAction SilentlyContinue | Stop-Process -Force

# Remove from Control Panel
Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\pw4you" -Force -ErrorAction SilentlyContinue

# Remove shortcuts
Remove-Item "`$env:APPDATA\Microsoft\Windows\Start Menu\Programs\$AppName" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "`$env:USERPROFILE\Desktop\$AppName.lnk" -Force -ErrorAction SilentlyContinue
Remove-Item "`$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup\$AppName.lnk" -Force -ErrorAction SilentlyContinue

# Remove file associations
Remove-Item "HKCU:\Software\Classes\.pw4" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "HKCU:\Software\Classes\pw4you.vault" -Recurse -Force -ErrorAction SilentlyContinue

# Ask about user data
`$result = [System.Windows.Forms.MessageBox]::Show(
    "是否同时删除所有用户数据（保险箱配置和锁定记录）？",
    "$AppName 卸载",
    [System.Windows.Forms.MessageBoxButtons]::YesNo,
    [System.Windows.Forms.MessageBoxIcon]::Question
)
if (`$result -eq 'Yes') {
    Remove-Item "`$env:APPDATA\pw4you" -Recurse -Force -ErrorAction SilentlyContinue
}

# Remove install directory
Start-Sleep -Seconds 1
Remove-Item `$InstallDir -Recurse -Force -ErrorAction SilentlyContinue

[System.Windows.Forms.MessageBox]::Show(
    "$AppName 已成功卸载。",
    "卸载完成",
    [System.Windows.Forms.MessageBoxButtons]::OK,
    [System.Windows.Forms.MessageBoxIcon]::Information
)
"@ | Out-File -FilePath "$destDir\uninstall.ps1" -Encoding UTF8
    Write-InstallLog "Created uninstaller"

    # 5. Create shortcuts
    $WshShell = New-Object -ComObject WScript.Shell

    if ($createStartMenu) {
        $startDir = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\$AppName"
        New-Item -ItemType Directory -Force -Path $startDir | Out-Null

        $shortcut = $WshShell.CreateShortcut("$startDir\$AppName.lnk")
        $shortcut.TargetPath = $installedExe
        $shortcut.WorkingDirectory = $destDir
        $shortcut.Description = "pw4you - 安全的文件夹加密工具"
        $shortcut.Save()

        # Uninstall shortcut
        $unShortcut = $WshShell.CreateShortcut("$startDir\卸载 $AppName.lnk")
        $unShortcut.TargetPath = "powershell.exe"
        $unShortcut.Arguments = "-ExecutionPolicy Bypass -WindowStyle Hidden -File `"$destDir\uninstall.ps1`""
        $unShortcut.WorkingDirectory = $destDir
        $unShortcut.Description = "卸载 $AppName"
        $unShortcut.Save()
    }

    if ($createDesktop) {
        $desktopDir = [Environment]::GetFolderPath("Desktop")
        $shortcut = $WshShell.CreateShortcut("$desktopDir\$AppName.lnk")
        $shortcut.TargetPath = $installedExe
        $shortcut.WorkingDirectory = $destDir
        $shortcut.Description = "pw4you - 安全的文件夹加密工具"
        $shortcut.Save()
    }

    if ($autoStart) {
        $startupDir = [Environment]::GetFolderPath("Startup")
        $shortcut = $WshShell.CreateShortcut("$startupDir\$AppName.lnk")
        $shortcut.TargetPath = $installedExe
        $shortcut.WorkingDirectory = $destDir
        $shortcut.WindowStyle = 7  # Minimized
        $shortcut.Save()
    }

    # 6. File association for .pw4
    if ($assocFiles) {
        # Register .pw4 extension
        New-Item "HKCU:\Software\Classes\.pw4" -Force | Out-Null
        Set-ItemProperty "HKCU:\Software\Classes\.pw4" -Name "(Default)" -Value "pw4you.vault"

        # Register progid
        New-Item "HKCU:\Software\Classes\pw4you.vault" -Force | Out-Null
        Set-ItemProperty "HKCU:\Software\Classes\pw4you.vault" -Name "(Default)" -Value "pw4you 保险箱"

        New-Item "HKCU:\Software\Classes\pw4you.vault\DefaultIcon" -Force | Out-Null
        Set-ItemProperty "HKCU:\Software\Classes\pw4you.vault\DefaultIcon" -Name "(Default)" -Value "$installedExe,0"

        New-Item "HKCU:\Software\Classes\pw4you.vault\shell\open\command" -Force | Out-Null
        Set-ItemProperty "HKCU:\Software\Classes\pw4you.vault\shell\open\command" -Name "(Default)" -Value "`"$installedExe`" `"%1`""
    }

    Write-InstallLog "Installation complete"
}

# ── GUI Forms ──────────────────────────────────────

# Main form
$form = New-Object System.Windows.Forms.Form
$form.Text = "pw4you v$AppVersion - 安装向导"
$form.Size = New-Object System.Drawing.Size(520, 420)
$form.StartPosition = "CenterScreen"
$form.FormBorderStyle = "FixedDialog"
$form.MaximizeBox = $false
$form.BackColor = $bgColor
$form.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 9)

# ── Page 1: Welcome ────────────────────────────────
$page1 = New-Object System.Windows.Forms.Panel
$page1.Size = $form.ClientSize
$page1.Location = New-Object System.Drawing.Point(0, 0)
$page1.BackColor = [System.Drawing.Color]::White

$title1 = New-Object System.Windows.Forms.Label
$title1.Text = "欢迎使用 pw4you 安装向导"
$title1.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 16, [System.Drawing.FontStyle]::Bold)
$title1.ForeColor = $accent
$title1.Location = New-Object System.Drawing.Point(30, 20)
$title1.Size = New-Object System.Drawing.Size(450, 40)
$page1.Controls.Add($title1)

$subtitle1 = New-Object System.Windows.Forms.Label
$subtitle1.Text = "文件夹保险箱 - 安全的文件夹加密工具"
$subtitle1.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 10)
$subtitle1.ForeColor = [System.Drawing.Color]::Gray
$subtitle1.Location = New-Object System.Drawing.Point(30, 60)
$subtitle1.Size = New-Object System.Drawing.Size(450, 25)
$page1.Controls.Add($subtitle1)

$desc1 = New-Object System.Windows.Forms.Label
$desc1.Text = @"
本向导将安装 pw4you v$AppVersion 到您的电脑上。

pw4you 是一款安全的文件夹加密工具，采用 AES-256-GCM
军用级加密保护您的文件。

主要功能：
  🔐 动态密码保护（日期 + 固定密码）
  🛡 防时钟篡改锁定机制
  📦 单文件容器，轻量便携（~4.5 MB）
  🔢 递增锁定时间（n² 天）

点击"下一步"继续。
"@
$desc1.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 9)
$desc1.Location = New-Object System.Drawing.Point(30, 95)
$desc1.Size = New-Object System.Drawing.Size(450, 200)
$desc1.ForeColor = $textColor
$page1.Controls.Add($desc1)

# ── Page 2: Options ────────────────────────────────
$page2 = New-Object System.Windows.Forms.Panel
$page2.Size = $form.ClientSize
$page2.Location = $page1.Location
$page2.BackColor = [System.Drawing.Color]::White
$page2.Visible = $false

$title2 = New-Object System.Windows.Forms.Label
$title2.Text = "选择安装选项"
$title2.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 14, [System.Drawing.FontStyle]::Bold)
$title2.ForeColor = $accent
$title2.Location = New-Object System.Drawing.Point(30, 20)
$title2.Size = New-Object System.Drawing.Size(450, 35)
$page2.Controls.Add($title2)

# Install path
$pathLabel = New-Object System.Windows.Forms.Label
$pathLabel.Text = "安装路径:"
$pathLabel.Location = New-Object System.Drawing.Point(30, 70)
$pathLabel.Size = New-Object System.Drawing.Size(100, 25)
$page2.Controls.Add($pathLabel)

$pathBox = New-Object System.Windows.Forms.TextBox
$pathBox.Text = $InstallDir
$pathBox.Location = New-Object System.Drawing.Point(30, 95)
$pathBox.Size = New-Object System.Drawing.Size(400, 25)
$page2.Controls.Add($pathBox)

$browseBtn = New-Object System.Windows.Forms.Button
$browseBtn.Text = "浏览..."
$browseBtn.Location = New-Object System.Drawing.Point(440, 94)
$browseBtn.Size = New-Object System.Drawing.Size(60, 26)
$browseBtn.Add_Click({
    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = "选择安装目录"
    $dialog.SelectedPath = $pathBox.Text
    if ($dialog.ShowDialog() -eq 'OK') {
        $pathBox.Text = Join-Path $dialog.SelectedPath $AppName
    }
})
$page2.Controls.Add($browseBtn)

# Options
$y = 140
$desktopCb = New-Object System.Windows.Forms.CheckBox
$desktopCb.Text = "创建桌面快捷方式"
$desktopCb.Checked = $true
$desktopCb.Location = New-Object System.Drawing.Point(30, $y)
$desktopCb.Size = New-Object System.Drawing.Size(300, 25)
$page2.Controls.Add($desktopCb)

$y += 30
$startMenuCb = New-Object System.Windows.Forms.CheckBox
$startMenuCb.Text = "创建开始菜单快捷方式"
$startMenuCb.Checked = $true
$startMenuCb.Location = New-Object System.Drawing.Point(30, $y)
$startMenuCb.Size = New-Object System.Drawing.Size(300, 25)
$page2.Controls.Add($startMenuCb)

$y += 30
$assocCb = New-Object System.Windows.Forms.CheckBox
$assocCb.Text = "关联 .pw4 文件（双击打开）"
$assocCb.Checked = $true
$assocCb.Location = New-Object System.Drawing.Point(30, $y)
$assocCb.Size = New-Object System.Drawing.Size(300, 25)
$page2.Controls.Add($assocCb)

$y += 30
$autoStartCb = New-Object System.Windows.Forms.CheckBox
$autoStartCb.Text = "开机自动启动（最小化到托盘）"
$autoStartCb.Checked = $false
$autoStartCb.Location = New-Object System.Drawing.Point(30, $y)
$autoStartCb.Size = New-Object System.Drawing.Size(300, 25)
$page2.Controls.Add($autoStartCb)

# ── Page 3: Installing ─────────────────────────────
$page3 = New-Object System.Windows.Forms.Panel
$page3.Size = $form.ClientSize
$page3.Location = $page1.Location
$page3.BackColor = [System.Drawing.Color]::White
$page3.Visible = $false

$title3 = New-Object System.Windows.Forms.Label
$title3.Text = "正在安装..."
$title3.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 14, [System.Drawing.FontStyle]::Bold)
$title3.ForeColor = $accent
$title3.Location = New-Object System.Drawing.Point(30, 20)
$title3.Size = New-Object System.Drawing.Size(450, 35)
$page3.Controls.Add($title3)

$progressBar = New-Object System.Windows.Forms.ProgressBar
$progressBar.Style = "Marquee"
$progressBar.Location = New-Object System.Drawing.Point(30, 80)
$progressBar.Size = New-Object System.Drawing.Size(450, 25)
$page3.Controls.Add($progressBar)

$statusLabel = New-Object System.Windows.Forms.Label
$statusLabel.Text = "正在准备安装..."
$statusLabel.Location = New-Object System.Drawing.Point(30, 115)
$statusLabel.Size = New-Object System.Drawing.Size(450, 25)
$page3.Controls.Add($statusLabel)

# ── Page 4: Complete ───────────────────────────────
$page4 = New-Object System.Windows.Forms.Panel
$page4.Size = $form.ClientSize
$page4.Location = $page1.Location
$page4.BackColor = [System.Drawing.Color]::White
$page4.Visible = $false

$title4 = New-Object System.Windows.Forms.Label
$title4.Text = "安装完成！"
$title4.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 16, [System.Drawing.FontStyle]::Bold)
$title4.ForeColor = $greenOk
$title4.Location = New-Object System.Drawing.Point(30, 20)
$title4.Size = New-Object System.Drawing.Size(450, 40)
$page4.Controls.Add($title4)

$desc4 = New-Object System.Windows.Forms.Label
$desc4.Text = @"
pw4you 已成功安装到您的电脑上。

您可以通过以下方式启动：
  📁 开始菜单 → pw4you
  🖥 桌面快捷方式（如已选择）
  📄 双击任意 .pw4 文件（如已关联）

要卸载，请使用：
  ⚙ 控制面板 → 程序和功能 → pw4you → 卸载
  或
  📁 开始菜单 → pw4you → 卸载 pw4you
"@
$desc4.Font = New-Object System.Drawing.Font("Microsoft YaHei UI", 9)
$desc4.Location = New-Object System.Drawing.Point(30, 70)
$desc4.Size = New-Object System.Drawing.Size(450, 200)
$desc4.ForeColor = $textColor
$page4.Controls.Add($desc4)

$launchCb = New-Object System.Windows.Forms.CheckBox
$launchCb.Text = "立即启动 pw4you"
$launchCb.Checked = $true
$launchCb.Location = New-Object System.Drawing.Point(30, 280)
$launchCb.Size = New-Object System.Drawing.Size(300, 25)
$page4.Controls.Add($launchCb)

# ── Buttons ────────────────────────────────────────
$buttonPanel = New-Object System.Windows.Forms.Panel
$buttonPanel.Size = New-Object System.Drawing.Size($form.ClientSize.Width, 50)
$buttonPanel.Location = New-Object System.Drawing.Point(0, $form.ClientSize.Height - 55)
$buttonPanel.BackColor = [System.Drawing.Color]::FromArgb(240, 240, 245)

$backBtn = New-Object System.Windows.Forms.Button
$backBtn.Text = "< 上一步"
$backBtn.Size = New-Object System.Drawing.Size(90, 30)
$backBtn.Location = New-Object System.Drawing.Point(280, 10)
$backBtn.Enabled = $false
$buttonPanel.Controls.Add($backBtn)

$nextBtn = New-Object System.Windows.Forms.Button
$nextBtn.Text = "下一步 >"
$nextBtn.Size = New-Object System.Drawing.Size(90, 30)
$nextBtn.Location = New-Object System.Drawing.Point(380, 10)
$nextBtn.BackColor = $accent
$nextBtn.ForeColor = [System.Drawing.Color]::White
$nextBtn.FlatStyle = "Flat"
$buttonPanel.Controls.Add($nextBtn)

$cancelBtn = New-Object System.Windows.Forms.Button
$cancelBtn.Text = "取消"
$cancelBtn.Size = New-Object System.Drawing.Size(80, 30)
$cancelBtn.Location = New-Object System.Drawing.Point(190, 10)
$buttonPanel.Controls.Add($cancelBtn)

$form.Controls.Add($buttonPanel)

# ── Navigation ─────────────────────────────────────
$currentPage = 0
$pages = @($page1, $page2, $page3, $page4)
foreach ($p in $pages) { $form.Controls.Add($p) }

function Show-Page($index) {
    foreach ($p in $pages) { $p.Visible = $false }
    $pages[$index].Visible = $true
    $script:currentPage = $index

    $backBtn.Enabled = ($index -gt 0 -and $index -lt 3)

    switch ($index) {
        0 { $nextBtn.Text = "下一步 >"; $cancelBtn.Visible = $true }
        1 { $nextBtn.Text = "安装 >"; $cancelBtn.Visible = $true }
        2 { $nextBtn.Visible = $false; $backBtn.Enabled = $false; $cancelBtn.Visible = $false }
        3 { $nextBtn.Text = "完成"; $cancelBtn.Visible = $false; $backBtn.Visible = $false }
    }
}

$cancelBtn.Add_Click({ $form.Close() })

$backBtn.Add_Click({
    if ($currentPage -gt 0) {
        Show-Page ($currentPage - 1)
    }
})

$nextBtn.Add_Click({
    switch ($currentPage) {
        0 { Show-Page 1 }
        1 {
            # Start installation
            Show-Page 2
            $form.Refresh()

            # Run install in a background job to keep UI responsive
            $job = Start-Job -ScriptBlock {
                param($dest, $desktop, $startMenu, $assoc, $autoStart, $srcExe, $scriptPath)

                # Re-define the install function in the job context
                function Install-App {
                    param($d, $ds, $sm, $af, $as)
                    New-Item -ItemType Directory -Force -Path $d | Out-Null
                    New-Item -ItemType Directory -Force -Path "$d\data" | Out-Null
                    Copy-Item $srcExe $d -Force

                    $instExe = Join-Path $d "pw4you.exe"

                    # Uninstall registry
                    $rp = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\pw4you"
                    New-Item -Path $rp -Force | Out-Null
                    Set-ItemProperty -Path $rp -Name "DisplayName" -Value "pw4you - 文件夹保险箱"
                    Set-ItemProperty -Path $rp -Name "DisplayVersion" -Value "0.1.0"
                    Set-ItemProperty -Path $rp -Name "Publisher" -Value "pw4you"
                    Set-ItemProperty -Path $rp -Name "DisplayIcon" -Value $instExe
                    Set-ItemProperty -Path $rp -Name "UninstallString" -Value "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$d\uninstall.ps1`""
                    Set-ItemProperty -Path $rp -Name "InstallLocation" -Value $d
                    Set-ItemProperty -Path $rp -Name "NoModify" -Value 0
                    Set-ItemProperty -Path $rp -Name "NoRepair" -Value 1

                    # Create uninstaller
                    $unContent = @"
`$installedExe = "$instExe"
Get-Process pw4you -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\pw4you" -Force -ErrorAction SilentlyContinue
Remove-Item "`$env:APPDATA\Microsoft\Windows\Start Menu\Programs\pw4you" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "`$env:USERPROFILE\Desktop\pw4you.lnk" -Force -ErrorAction SilentlyContinue
Remove-Item "`$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup\pw4you.lnk" -Force -ErrorAction SilentlyContinue
Remove-Item "HKCU:\Software\Classes\.pw4" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "HKCU:\Software\Classes\pw4you.vault" -Recurse -Force -ErrorAction SilentlyContinue
`$r = [System.Windows.Forms.MessageBox]::Show('是否同时删除所有用户数据？','pw4you 卸载','YesNo','Question')
if (`$r -eq 'Yes') { Remove-Item "`$env:APPDATA\pw4you" -Recurse -Force -ErrorAction SilentlyContinue }
Start-Sleep 1
Remove-Item "$d" -Recurse -Force -ErrorAction SilentlyContinue
[System.Windows.Forms.MessageBox]::Show('pw4you 已成功卸载。','卸载完成','OK','Information')
"@
                    $unContent | Out-File "$d\uninstall.ps1" -Encoding UTF8

                    # Shortcuts
                    $ws = New-Object -ComObject WScript.Shell
                    if ($sm) {
                        $sd = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\pw4you"
                        New-Item -ItemType Directory -Force -Path $sd | Out-Null
                        $sc = $ws.CreateShortcut("$sd\pw4you.lnk")
                        $sc.TargetPath = $instExe; $sc.WorkingDirectory = $d
                        $sc.Description = "pw4you - 安全的文件夹加密工具"; $sc.Save()
                        $sc2 = $ws.CreateShortcut("$sd\卸载 pw4you.lnk")
                        $sc2.TargetPath = "powershell.exe"
                        $sc2.Arguments = "-ExecutionPolicy Bypass -WindowStyle Hidden -File `"$d\uninstall.ps1`""
                        $sc2.WorkingDirectory = $d; $sc2.Save()
                    }
                    if ($ds) {
                        $sc = $ws.CreateShortcut("$env:USERPROFILE\Desktop\pw4you.lnk")
                        $sc.TargetPath = $instExe; $sc.WorkingDirectory = $d
                        $sc.Description = "pw4you - 安全的文件夹加密工具"; $sc.Save()
                    }
                    if ($as) {
                        $sc = $ws.CreateShortcut([Environment]::GetFolderPath('Startup') + "\pw4you.lnk")
                        $sc.TargetPath = $instExe; $sc.WorkingDirectory = $d
                        $sc.WindowStyle = 7; $sc.Save()
                    }
                    if ($af) {
                        New-Item "HKCU:\Software\Classes\.pw4" -Force | Out-Null
                        Set-ItemProperty "HKCU:\Software\Classes\.pw4" -Name "(Default)" -Value "pw4you.vault"
                        New-Item "HKCU:\Software\Classes\pw4you.vault" -Force | Out-Null
                        Set-ItemProperty "HKCU:\Software\Classes\pw4you.vault" -Name "(Default)" -Value "pw4you 保险箱"
                        New-Item "HKCU:\Software\Classes\pw4you.vault\DefaultIcon" -Force | Out-Null
                        Set-ItemProperty "HKCU:\Software\Classes\pw4you.vault\DefaultIcon" -Name "(Default)" -Value "$instExe,0"
                        New-Item "HKCU:\Software\Classes\pw4you.vault\shell\open\command" -Force | Out-Null
                        Set-ItemProperty "HKCU:\Software\Classes\pw4you.vault\shell\open\command" -Name "(Default)" -Value "`"$instExe`" `"%1`""
                    }
                    return "OK"
                }
                Install-App -d $dest -ds $desktop -sm $startMenu -af $assoc -as $autoStart
            } -ArgumentList $pathBox.Text, $desktopCb.Checked, $startMenuCb.Checked, $assocCb.Checked, $autoStartCb.Checked, $SourceExe, $PSScriptRoot

            # Wait for job
            $job | Wait-Job | Out-Null
            $result = $job | Receive-Job
            $job | Remove-Job

            if ($result -eq "OK") {
                Show-Page 3
            } else {
                [System.Windows.Forms.MessageBox]::Show("安装失败: $result", "错误", "OK", "Error")
                Show-Page 1
            }
        }
        3 {
            if ($launchCb.Checked) {
                Start-Process (Join-Path $pathBox.Text $AppExe)
            }
            $form.Close()
        }
    }
})

# ── Start ──────────────────────────────────────────
Show-Page 0
[void] $form.ShowDialog()
