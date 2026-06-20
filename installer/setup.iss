; pw4you - Inno Setup Installer Script
; -----------------------------------------
; Generates a professional GUI installation wizard.
;
; Requirements: Inno Setup 6+ (free: https://jrsoftware.org/isinfo.php)
; Usage: Open this file in Inno Setup Compiler and click "Compile"
;        Or: iscc setup.iss

#define AppName        "pw4you"
#define AppVersion     "0.1.0"
#define AppPublisher   "pw4you"
#define AppURL         "https://github.com/pw4you/pw4you"
#define AppExeName     "pw4you.exe"
#define AppAssocName   "pw4you.vault"
#define AppAssocExt    ".pw4"

[Setup]
; Basic info
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}

; Install directory
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes

; Output
OutputDir=..\dist
OutputBaseFilename=pw4you-setup-{#AppVersion}
SetupIconFile=..\desktop\assets\icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern

; Windows version requirements
MinVersion=10.0

; Uninstall settings
UninstallDisplayName={#AppName} - 文件夹保险箱
UninstallDisplayIcon={app}\{#AppExeName}
Uninstallable=yes
CreateUninstallRegKey=yes

; Privileges
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog

; Appearance (uses modern Windows 10/11 style by default)

[Languages]
Name: "chinesesimp"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
; Customize wizard messages
chinesesimp.WelcomeLabel2=即将安装 [name] v{#AppVersion} 到您的电脑上。%n%n推荐在继续之前关闭所有其他应用程序。
chinesesimp.FinishedLabel=安装完成。点击"完成"退出安装向导。

[Types]
Name: "full"; Description: "完整安装"
Name: "custom"; Description: "自定义安装"; Flags: iscustom

[Components]
Name: "app"; Description: "主程序 (必需)"; Types: full custom; Flags: fixed
Name: "shortcuts"; Description: "开始菜单快捷方式"; Types: full
Name: "desktop"; Description: "桌面快捷方式"; Types: full
Name: "assoc"; Description: "关联 .pw4 文件"; Types: full

[Tasks]
Name: "autostart"; Description: "开机自动启动 (最小化到托盘)"; GroupDescription: "其他选项:"

[Files]
; Main executable
Source: "..\target\release\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion; Components: app

; Assets / data directory (created at runtime if needed)
; No extra files needed - the app creates its own data folder

[Icons]
; Start Menu
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Components: shortcuts
Name: "{group}\卸载 {#AppName}"; Filename: "{uninstallexe}"; Components: shortcuts

; Desktop
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Components: desktop

; Startup folder
Name: "{userstartup}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Parameters: "--minimized"; Tasks: autostart

[Registry]
; File association: .pw4 → pw4you.vault
Root: HKCU; Subkey: "Software\Classes\{#AppAssocExt}"; ValueType: string; ValueName: ""; ValueData: "{#AppAssocName}"; Flags: uninsdeletevalue; Components: assoc
Root: HKCU; Subkey: "Software\Classes\{#AppAssocExt}"; ValueType: string; ValueName: "PerceivedType"; ValueData: "Document"; Flags: uninsdeletevalue; Components: assoc

; ProgID: pw4you.vault
Root: HKCU; Subkey: "Software\Classes\{#AppAssocName}"; ValueType: string; ValueName: ""; ValueData: "pw4you 保险箱"; Flags: uninsdeletekey; Components: assoc
Root: HKCU; Subkey: "Software\Classes\{#AppAssocName}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName},0"; Components: assoc
Root: HKCU; Subkey: "Software\Classes\{#AppAssocName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Components: assoc

; Application registration (for "Open With")
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: "{#AppAssocExt}"; ValueData: ""; Components: assoc

; App Paths (for Windows search / Run dialog)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExeName}"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExeName}"; ValueType: string; ValueName: "Path"; ValueData: "{app}"; Flags: uninsdeletekey

[Run]
; Launch the app after installation
Filename: "{app}\{#AppExeName}"; Description: "启动 {#AppName}"; Flags: nowait postinstall shellexec skipifsilent

[UninstallRun]
; Kill running instance before uninstall
Filename: "taskkill"; Parameters: "/F /IM {#AppExeName}"; Flags: runhidden

[Code]
// Custom uninstall cleanup
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // Clean up app data (user is asked in the uninstaller)
    if MsgBox('是否同时删除所有用户数据（包括配置和锁定记录）？', mbConfirmation, MB_YESNO) = IDYES then
    begin
      DelTree(ExpandConstant('{userappdata}\{#AppName}'), True, True, True);
    end;
  end;
end;

// Verify the application is not running before install
function InitializeSetup(): Boolean;
var
  ResultCode: Integer;
begin
  Result := True;
  // Check if app is running
  if CheckForMutexes('pw4you_app_mutex') then
  begin
    if MsgBox('检测到 pw4you 正在运行。' + #13#10 +
              '请先关闭应用程序后再继续安装。' + #13#10#13#10 +
              '是否要重试？', mbError, MB_RETRYCANCEL) = IDRETRY then
    begin
      Result := InitializeSetup();
    end
    else
      Result := False;
  end;
end;

// Custom welcome page message
procedure InitializeWizard();
begin
  WizardForm.WelcomeLabel2.Width := WizardForm.WelcomeLabel2.Width + 100;
end;
