<#
.SYNOPSIS
  Builds the release exe and the Inno Setup installer into dist\.
.PARAMETER OfficialMsi
  Optional path to the official EPOS Gaming Suite MSI. When given, the audio driver files
  (eposaudio.inf, .sys, .cat, EAPO.dll) are extracted and bundled so the installer can offer
  to install the driver. They are EPOS's files: do not distribute the result.
#>
param([string]$OfficialMsi = "")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
& $cargo build --release -p eposcontrol
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$staging = Join-Path $root "installer\staging"
if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
New-Item -ItemType Directory -Force $staging | Out-Null
Copy-Item (Join-Path $root "target\release\eposcontrol.exe") $staging

if ($OfficialMsi -and (Test-Path $OfficialMsi)) {
    $tmp = Join-Path $env:TEMP ("epos_msi_" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $tmp | Out-Null
    Start-Process msiexec -ArgumentList "/a", "`"$OfficialMsi`"", "/qn", "TARGETDIR=`"$tmp`"" -Wait
    $drv = Get-ChildItem $tmp -Recurse -Directory -Filter GSADriver | Select-Object -First 1
    if ($drv) {
        $dst = Join-Path $staging "driver"
        New-Item -ItemType Directory -Force $dst | Out-Null
        foreach ($f in "eposaudio.inf", "eposaudio.sys", "eposaudio.cat", "EAPO.dll") {
            Copy-Item (Join-Path $drv.FullName $f) $dst
        }
        Write-Host "Bundled driver files from $($drv.FullName)"
    }
    Remove-Item $tmp -Recurse -Force
}

$version = (Select-String -Path (Join-Path $root "crates\app\Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$iscc = Get-ChildItem "$env:LOCALAPPDATA\Programs\Inno Setup 6", "C:\Program Files (x86)\Inno Setup 6", "C:\Program Files\Inno Setup 6" -Filter ISCC.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup 6 not found. Install it with: winget install JRSoftware.InnoSetup" }
& $iscc.FullName "/DAppVersion=$version" (Join-Path $root "installer\eposcontrol.iss")
if ($LASTEXITCODE -ne 0) { throw "ISCC failed" }
Get-ChildItem (Join-Path $root "dist") -Filter "EposControl-Setup-*.exe" | Select-Object Name, @{n = "MB"; e = { [math]::Round($_.Length / 1MB, 2) } }
