$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
$Version = $Metadata.packages[0].version
$Host = (rustc -vV | Select-String '^host: ').Line.Replace('host: ', '')
$Target = if ($args.Count -gt 0) { $args[0] } else { $Host }
$Package = "savemyterminal-$Version-$Target"
$Stage = Join-Path $Root "dist/stage/$Package"
$Archive = Join-Path $Root "dist/$Package.zip"

if ($Target -eq $Host) {
    cargo build --locked --release
    $Binary = Join-Path $Root "target/release/savemyterminal.exe"
} else {
    cargo build --locked --release --target $Target
    $Binary = Join-Path $Root "target/$Target/release/savemyterminal.exe"
}

if (-not (Test-Path $Binary)) { throw "Release binary not found: $Binary" }
Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item (Join-Path $Stage "docs") -ItemType Directory -Force | Out-Null
Copy-Item $Binary (Join-Path $Stage "savemyterminal.exe")
Copy-Item README.md, LICENSE, CHANGELOG.md $Stage
Copy-Item docs/compatibility.md (Join-Path $Stage "docs/compatibility.md")

New-Item (Split-Path -Parent $Archive) -ItemType Directory -Force | Out-Null
Remove-Item $Archive -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $Stage -DestinationPath $Archive
$Hash = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
"$Hash  $(Split-Path -Leaf $Archive)" | Set-Content "$Archive.sha256" -Encoding ascii

Write-Output $Archive
Write-Output "$Archive.sha256"
