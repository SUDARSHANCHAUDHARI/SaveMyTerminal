$ErrorActionPreference = "Stop"

$Version = if ($env:SAVEMYTERMINAL_VERSION) { $env:SAVEMYTERMINAL_VERSION } else { "1.0.1" }
$Target = if ($env:SAVEMYTERMINAL_TARGET) {
    $env:SAVEMYTERMINAL_TARGET
} elseif ($env:PROCESSOR_ARCHITECTURE -eq "AMD64") {
    "x86_64-pc-windows-msvc"
} else {
    throw "Unsupported platform; set SAVEMYTERMINAL_TARGET explicitly"
}
$InstallDir = if ($env:SAVEMYTERMINAL_INSTALL_DIR) { $env:SAVEMYTERMINAL_INSTALL_DIR } else { Join-Path $HOME ".local/bin" }
$Temporary = $null
$Extract = $null

try {
    if ($args.Count -gt 0) {
        $Archive = (Resolve-Path $args[0]).Path
    } else {
        $Temporary = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid()))
        $Name = "savemyterminal-$Version-$Target.zip"
        $Base = "https://github.com/SUDARSHANCHAUDHARI/SaveMyTerminal/releases/download/v$Version"
        $Archive = Join-Path $Temporary $Name
        Invoke-WebRequest "$Base/$Name" -OutFile $Archive
        Invoke-WebRequest "$Base/$Name.sha256" -OutFile "$Archive.sha256"
    }

    $Expected = (Get-Content "$Archive.sha256").Split(' ')[0].Trim().ToLowerInvariant()
    $Actual = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Expected -ne $Actual) { throw "SHA256 checksum mismatch" }

    $Extract = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid()))
    Expand-Archive $Archive $Extract
    $Binary = Get-ChildItem $Extract -Filter savemyterminal.exe -Recurse | Select-Object -First 1
    if (-not $Binary) { throw "savemyterminal.exe was not found in the archive" }
    New-Item $InstallDir -ItemType Directory -Force | Out-Null
    Copy-Item $Binary.FullName (Join-Path $InstallDir "savemyterminal.exe") -Force
    Write-Output "Installed $(Join-Path $InstallDir 'savemyterminal.exe')"
} finally {
    if ($Temporary) { Remove-Item $Temporary -Recurse -Force }
    if ($Extract) { Remove-Item $Extract -Recurse -Force }
}
