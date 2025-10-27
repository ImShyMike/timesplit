param(
    [string]$Command = 'help'
)

$ProgramName = 'timesplit'
$InstallDir = Join-Path -Path ${env:ProgramFiles} -ChildPath $ProgramName
$InstallPath = Join-Path -Path $InstallDir -ChildPath "$ProgramName.exe"
$TaskName = 'TimeSplit'
$GhApi = 'https://api.github.com/repos/ImShyMike/timesplit/releases/latest'

function Write-Info([string]$m){ Write-Host "-> $m" -ForegroundColor Yellow }
function Write-Success([string]$m){ Write-Host "[OK] $m" -ForegroundColor Green }
function Write-Err([string]$m){ Write-Host "[ERR] $m" -ForegroundColor Red }

function Ensure-Admin {
    $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        Write-Err "This script must be run as Administrator (open PowerShell as Admin)."
        exit 1
    }
}

function Get-DownloadInfo {
    Write-Info "Querying GitHub for latest release..."
    try {
        $release = Invoke-RestMethod -Uri $GhApi -UseBasicParsing -ErrorAction Stop
    } catch {
        Write-Err "Failed to fetch release information from GitHub: $_"
        return $null
    }

    $tag = $release.tag_name
    if (-not $tag) {
        Write-Err "Unable to determine release tag name from GitHub response."
        return $null
    }

    $version = $tag.TrimStart('v')
    $expectedName = "$ProgramName-$version-x86_64-pc-windows-gnu.exe"

    $asset = $release.assets | Where-Object { $_.name -eq $expectedName } | Select-Object -First 1

    if ($null -eq $asset) {
        Write-Err "Expected asset '$expectedName' not found in release '$tag'."
        return $null
    }

    Write-Info "Selected asset: $($asset.name)"
    return [PSCustomObject]@{
        Url = $asset.browser_download_url
        Name = $asset.name
        Version = $version
    }
}

function Create-ScheduledTask-PowerShell {
    param([string]$exePath)
    try {
        $action = New-ScheduledTaskAction -Execute $exePath -Argument 'run'
        $trigger = New-ScheduledTaskTrigger -AtStartup
        Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -RunLevel Highest -User 'SYSTEM' -Force | Out-Null
        Write-Success "Scheduled Task '$TaskName' created (PowerShell ScheduledTask API)."
        Start-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
        return $true
    } catch {
        Write-Err "PowerShell ScheduledTask creation failed: $_"
        return $false
    }
}

function Create-ScheduledTask-Schtasks {
    param([string]$exePath)
    try {
        $quoted = '"' + $exePath + '" run'
        $cmd = "schtasks /Create /SC ONSTART /TN `"$TaskName`" /TR $quoted /RL HIGHEST /F /RU SYSTEM"
        Write-Info "Running: $cmd"
        $proc = Start-Process -FilePath schtasks -ArgumentList "/Create","/SC","ONSTART","/TN","$TaskName","/TR",$quoted,"/RL","HIGHEST","/F","/RU","SYSTEM" -NoNewWindow -PassThru -Wait -ErrorAction Stop
        Write-Success "Scheduled Task '$TaskName' created (schtasks)."
        return $true
    } catch {
        Write-Err "schtasks creation failed: $_"
        return $false
    }
}

function Install-Program {
    Ensure-Admin

    if (-not [Environment]::Is64BitOperatingSystem) {
        Write-Err "Unsupported architecture. Windows releases are currently available only for x86_64."
        exit 1
    }

    $downloadInfo = Get-DownloadInfo
    if (-not $downloadInfo) { exit 1 }

    $tmp = Join-Path -Path $env:TEMP -ChildPath $downloadInfo.Name
    Write-Info "Downloading $ProgramName $($downloadInfo.Version) from $($downloadInfo.Url) to $tmp"
    try {
        Invoke-WebRequest -Uri $downloadInfo.Url -OutFile $tmp -UseBasicParsing -ErrorAction Stop
    } catch {
        Write-Err "Download failed: $_"
        exit 1
    }

    Write-Info "Installing to $InstallDir"
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    try {
        Move-Item -Path $tmp -Destination $InstallPath -Force
    } catch {
        Write-Err "Failed to move binary into place: $_"
        exit 1
    }

    Write-Success "Binary installed to $InstallPath (version $($downloadInfo.Version))"

    # Try PowerShell ScheduledTask API first, fallback to schtasks.exe
    if (Get-Command -Name Register-ScheduledTask -ErrorAction SilentlyContinue) {
        if (-not (Create-ScheduledTask-PowerShell -exePath $InstallPath)) {
            Write-Info "Falling back to schtasks.exe"
            Create-ScheduledTask-Schtasks -exePath $InstallPath | Out-Null
        }
    } else {
        Write-Info 'Register-ScheduledTask not available; using schtasks.exe'
        Create-ScheduledTask-Schtasks -exePath $InstallPath | Out-Null
    }

    Write-Success "$ProgramName installed and scheduled to run at startup."
    Write-Info "Use 'install.ps1 status' to check the installation."
}

function Uninstall-Program {
    Ensure-Admin

    Write-Info "Removing scheduled task (if present)"
    if (Get-Command -Name Unregister-ScheduledTask -ErrorAction SilentlyContinue) {
        try { Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue }
        catch { }
    }

    # Also try schtasks delete
    try { schtasks /Delete /TN $TaskName /F > $null 2>&1 } catch { }

    if (Test-Path $InstallDir) {
        try {
            Remove-Item -Path $InstallDir -Recurse -Force
            Write-Success "Removed $InstallDir"
        } catch {
            Write-Err "Failed to remove ${InstallDir}: $_"
        }
    } else {
        Write-Info "$InstallDir not present"
    }

    Write-Success "$ProgramName uninstalled."
}

function Show-Status {
    if (Test-Path $InstallPath) {
        Write-Success "$ProgramName is installed at $InstallPath"
    } else {
        Write-Err "$ProgramName is not installed"
    }

    Write-Info "Scheduled task status:"
    if (Get-Command -Name Get-ScheduledTask -ErrorAction SilentlyContinue) {
        try {
            $task = Get-ScheduledTask -TaskName $TaskName -ErrorAction Stop
            $task | Format-List | Out-Host
        } catch {
            Write-Info "Scheduled task '$TaskName' not found via PowerShell API."
        }
    }

    try {
        schtasks /Query /TN $TaskName 2>$null
    } catch {
        Write-Info "No schtasks entry for $TaskName"
    }
}

function Show-Usage {
    Write-Host "Usage: install.ps1 [install|uninstall|update|status|help]"
}

switch ($Command.ToLower()) {
    'install' { Install-Program }
    'uninstall' { Uninstall-Program }
    'update' { Uninstall-Program; Install-Program }
    'status' { Show-Status }
    'help' { Show-Usage }
    default { Write-Err ("Invalid command: {0}`n" -f $Command); Show-Usage; exit 1 }
}
