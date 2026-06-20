# Generate pw4you app icon (.ico)
# Creates a simple lock icon programmatically.

param(
    [string]$OutputPath = "desktop\assets\icon.ico"
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Resolve-Path "$scriptDir\.."
$outputAbs = Join-Path $projectRoot $OutputPath

$outputDir = Split-Path -Parent $outputAbs
if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
}

Add-Type -AssemblyName System.Drawing

$iconSize = 32
$bitmap = New-Object System.Drawing.Bitmap($iconSize, $iconSize)
$g = [System.Drawing.Graphics]::FromImage($bitmap)
$g.SmoothingMode = 'HighQuality'

# Lock body colors
$darkBlue  = [System.Drawing.Color]::FromArgb(255, 25, 55, 110)
$medBlue   = [System.Drawing.Color]::FromArgb(255, 40, 80, 160)
$lightBlue = [System.Drawing.Color]::FromArgb(255, 80, 140, 220)
$keyColor  = [System.Drawing.Color]::FromArgb(255, 210, 215, 225)
$white     = [System.Drawing.Color]::White

$darkBrush   = New-Object System.Drawing.SolidBrush($darkBlue)
$medBrush    = New-Object System.Drawing.SolidBrush($medBlue)
$lightBrush  = New-Object System.Drawing.SolidBrush($lightBlue)
$keyBrush    = New-Object System.Drawing.SolidBrush($keyColor)

# Shackle (top arc)
$g.FillEllipse($darkBrush, 3, 0, 26, 24)
$g.FillEllipse($medBrush, 5, 2, 22, 20)
$g.FillEllipse($lightBrush, 7, 4, 18, 16)

# Shackle inner cutout (transparent hole)
$transBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::Transparent)
$g.FillEllipse($transBrush, 10, 7, 12, 10)
$g.FillRectangle($transBrush, 10, 12, 12, 8)

# Lock body (bottom rectangle)
$g.FillRectangle($darkBrush, 4, 14, 24, 17)
$g.FillRectangle($medBrush, 6, 16, 20, 13)

# Keyhole
$g.FillEllipse($keyBrush, 13, 18, 6, 6)
$g.FillRectangle($keyBrush, 14, 22, 4, 5)

# Highlights
$highlightPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(120, 150, 210, 255), 1.5)
$g.DrawArc($highlightPen, 5, 1, 22, 20, 210, 120)
$g.DrawLine($highlightPen, 7, 17, 7, 25)

$g.Dispose()
$darkBrush.Dispose(); $medBrush.Dispose(); $lightBrush.Dispose()
$keyBrush.Dispose(); $transBrush.Dispose(); $highlightPen.Dispose()

# Convert to Icon and save
$icon = [System.Drawing.Icon]::FromHandle($bitmap.GetHicon())
$fileStream = [System.IO.File]::OpenWrite($outputAbs)
$icon.Save($fileStream)
$fileStream.Close()

$bitmap.Dispose()
$icon.Dispose()

Write-Output "Icon generated: $outputAbs ($((Get-Item $outputAbs).Length) bytes)"
