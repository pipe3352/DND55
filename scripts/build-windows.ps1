# Builds DND55 natively on Windows 10/11 (MSVC toolchain) and packages it.
# Usage (PowerShell, in the project folder):   .\scripts\build-windows.ps1
# Optional: install Inno Setup 6 (https://jrsoftware.org/isinfo.php) to also produce DND55-Setup.exe.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches.Groups[1].Value

Write-Host "Building DND55 $version (release)..." -ForegroundColor Yellow
cargo build --release -p dnd_desktop
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Remove-Item -Recurse -Force dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force dist\DND55 | Out-Null
Copy-Item target\release\DND55.exe dist\DND55\
Copy-Item WINDOWS_INSTALLATION_GUIDE.md dist\DND55\
Copy-Item packaging\windows\README.txt dist\DND55\README.txt
Get-Content LICENSE-MIT, packaging\windows\THIRD_PARTY.txt | Set-Content dist\DND55\LICENSE.txt

# Portable zip
New-Item -ItemType Directory -Force dist\portable\DND55 | Out-Null
Copy-Item dist\DND55\* dist\portable\DND55\
"This file makes DND55 portable: saves go to the data folder next to DND55.exe." | Set-Content dist\portable\DND55\portable.txt
Compress-Archive -Path dist\portable\DND55 -DestinationPath dist\DND55-Windows-Portable.zip -Force
Remove-Item -Recurse -Force dist\portable

# Installer (optional)
$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (Test-Path $iscc) {
    $env:DND55_VERSION = $version
    & $iscc packaging\windows\DND55.iss
} elseif (Get-Command makensis -ErrorAction SilentlyContinue) {
    makensis "-DVERSION=$version" "-DSTAGE=$PWD\dist\DND55" "-DOUTFILE=$PWD\dist\DND55-Setup.exe" packaging\windows\installer.nsi
} else {
    Write-Host "Inno Setup / NSIS not found: skipped DND55-Setup.exe (the portable zip works without it)." -ForegroundColor DarkYellow
}
Get-ChildItem dist
