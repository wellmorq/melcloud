[CmdletBinding()]
param(
    [string]$OutputDir = "build",
    [switch]$NoEnv,
    [switch]$NoRuntimeState
)

$ErrorActionPreference = "Stop"

$root = $PSScriptRoot
if (-not $root) {
    $root = (Get-Location).Path
}

$root = [System.IO.Path]::GetFullPath($root)
$outputPath = [System.IO.Path]::GetFullPath((Join-Path $root $OutputDir))
$rootPrefix = if ($root.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
    $root
} else {
    $root + [System.IO.Path]::DirectorySeparatorChar
}

if (
    $outputPath.Equals($root, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not $outputPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)
) {
    throw "OutputDir must be a child directory inside the repository root: $OutputDir"
}

function Copy-RequiredFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Required file is missing: $Source"
    }
    $parent = Split-Path -Parent $Destination
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Copy-RequiredDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
        throw "Required directory is missing: $Source"
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Destination) | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force
}

function Write-TextFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Content
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    Set-Content -LiteralPath $Path -Value $Content -Encoding UTF8
}

Push-Location $root
try {
    cargo build --release --workspace
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    if (Test-Path -LiteralPath $outputPath) {
        Remove-Item -LiteralPath $outputPath -Recurse -Force
    }

    New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "bin") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-site") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-cli") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-cli\presets") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-cli\state") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-site\state") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-site\cache") | Out-Null

    $isWindowsRuntime = ($PSVersionTable.PSEdition -eq "Desktop") -or ($env:OS -eq "Windows_NT") -or ($IsWindows -eq $true)
    $exeSuffix = if ($isWindowsRuntime) { ".exe" } else { "" }
    $releaseDir = Join-Path $root "target\release"

    Copy-RequiredFile `
        -Source (Join-Path $releaseDir "melcloud-cli$exeSuffix") `
        -Destination (Join-Path $outputPath "bin\melcloud-cli$exeSuffix")
    Copy-RequiredFile `
        -Source (Join-Path $releaseDir "melcloud-site$exeSuffix") `
        -Destination (Join-Path $outputPath "bin\melcloud-site$exeSuffix")

    Copy-RequiredDirectory `
        -Source (Join-Path $root "melcloud-site\public") `
        -Destination (Join-Path $outputPath "melcloud-site\public")
    $assetVersion = Get-Date -Format "yyyyMMddHHmmss"
    Write-TextFile -Path (Join-Path $outputPath "melcloud-site\public\js\build-version.js") -Content @"
export const assetVersion = "$assetVersion";
"@
    Copy-RequiredDirectory `
        -Source (Join-Path $root "melcloud-site\site-assets") `
        -Destination (Join-Path $outputPath "melcloud-site\site-assets")
    Copy-RequiredFile `
        -Source (Join-Path $root "melcloud-site\melcloud-site.yaml") `
        -Destination (Join-Path $outputPath "melcloud-site\melcloud-site.yaml")

    $envPath = Join-Path $root ".env"
    if (-not $NoEnv -and (Test-Path -LiteralPath $envPath -PathType Leaf)) {
        Copy-RequiredFile -Source $envPath -Destination (Join-Path $outputPath ".env")
    } else {
        Write-TextFile -Path (Join-Path $outputPath ".env.example") -Content @"
login=your@email
password=your-password
language=ru
"@
    }

    $cliPresetSource = Join-Path $root "melcloud-cli\presets"
    $cliPresetDestination = Join-Path $outputPath "melcloud-cli\presets"
    if (-not $NoRuntimeState -and (Test-Path -LiteralPath $cliPresetSource -PathType Container)) {
        Get-ChildItem -LiteralPath $cliPresetSource -Force |
            ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $cliPresetDestination -Recurse -Force
            }
    }

    $cliStateSource = Join-Path $root "melcloud-cli\state"
    $cliStateDestination = Join-Path $outputPath "melcloud-cli\state"
    if (-not $NoRuntimeState -and (Test-Path -LiteralPath $cliStateSource -PathType Container)) {
        Get-ChildItem -LiteralPath $cliStateSource -Force |
            ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $cliStateDestination -Recurse -Force
            }
    }

    $siteStateSource = Join-Path $root "melcloud-site\state"
    $siteStateDestination = Join-Path $outputPath "melcloud-site\state"
    if (-not $NoRuntimeState -and (Test-Path -LiteralPath $siteStateSource -PathType Container)) {
        Get-ChildItem -LiteralPath $siteStateSource -Force |
            ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $siteStateDestination -Recurse -Force
            }
    }

    $siteCacheSource = Join-Path $root "melcloud-site\cache"
    $siteCacheDestination = Join-Path $outputPath "melcloud-site\cache"
    if (-not $NoRuntimeState -and (Test-Path -LiteralPath $siteCacheSource -PathType Container)) {
        Get-ChildItem -LiteralPath $siteCacheSource -Force |
            ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $siteCacheDestination -Recurse -Force
            }
    }

    New-Item -ItemType Directory -Force -Path (Join-Path $outputPath "melcloud-site\cache\weather-icons") | Out-Null

    if ($isWindowsRuntime) {
        Write-TextFile -Path (Join-Path $outputPath "run-site.cmd") -Content @"
@echo off
cd /d "%~dp0"
bin\melcloud-site.exe
"@
    }

    Write-TextFile -Path (Join-Path $outputPath "README_RUNTIME.txt") -Content @"
MelCloud runtime package

Run:
  bin\melcloud-site$exeSuffix

Open:
  http://127.0.0.1:8787/

Runtime files:
  .env
  bin\
  melcloud-cli\presets\
  melcloud-cli\state\
  melcloud-site\state\
  melcloud-site\cache\
  melcloud-site\melcloud-site.yaml

The site calls bin\melcloud-cli$exeSuffix from this same folder.
"@

    Write-Host "Runtime package created: $outputPath"
}
finally {
    Pop-Location
}
