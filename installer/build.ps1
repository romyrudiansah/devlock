# Bangun installer NSIS dari folder paket rilis (berisi bin\*.exe, README.md, CHANGELOG.md, LICENSE-*).
# Dipakai workflow rilis dan CI (CI memakai binary tiruan hanya untuk mengecek skrip).
param(
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $Source,
    [Parameter(Mandatory)] [string] $Output
)
$ErrorActionPreference = 'Stop'

$makensis = (Get-Command makensis.exe -ErrorAction SilentlyContinue).Source
if (-not $makensis) {
    $makensis = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
}
if (-not (Test-Path $makensis)) {
    choco install nsis -y --no-progress | Out-Null
    $makensis = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
}

$script = Join-Path $PSScriptRoot 'devlock.nsi'
& $makensis /V2 "/DVERSION=$Version" "/DSRC=$((Resolve-Path $Source).Path)" "/DOUT=$Output" $script
if ($LASTEXITCODE -ne 0) { throw "makensis gagal ($LASTEXITCODE)" }
Write-Output "Installer: $Output"
