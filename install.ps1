param(
    [string]$Command = 'help'
)

$ProgramName = 'timesplit'
$InstallDir = Join-Path -Path ${env:ProgramFiles} -ChildPath $ProgramName
$InstallPath = Join-Path -Path $InstallDir -ChildPath "$ProgramName.exe"
$StartupFolder = [Environment]::GetFolderPath('Startup')
$ShortcutPath = Join-Path -Path $StartupFolder -ChildPath "$ProgramName.lnk"
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

function Add-ToPath {
    param([string]$directory)
    
    try {
        $currentPath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
        
        if ($currentPath -split ';' | Where-Object { $_ -eq $directory }) {
            Write-Info "$directory is already in PATH"
            return
        }
        
        $newPath = $currentPath.TrimEnd(';') + ';' + $directory
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'Machine')
        Write-Success "Added $directory to system PATH"
        Write-Info "You may need to restart your terminal for PATH changes to take effect"
    } catch {
        Write-Err "Failed to add to PATH: $_"
    }
}

function Remove-FromPath {
    param([string]$directory)
    
    try {
        $currentPath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
        $pathArray = $currentPath -split ';' | Where-Object { $_ -and $_ -ne $directory }
        $newPath = $pathArray -join ';'
        
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'Machine')
        Write-Success "Removed $directory from system PATH"
    } catch {
        Write-Err "Failed to remove from PATH: $_"
    }
}

function Create-StartupShortcut {
    param([string]$targetPath)
    
    try {
        $WScriptShell = New-Object -ComObject WScript.Shell
        $Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
        $Shortcut.TargetPath = $targetPath
        $Shortcut.Arguments = "run"
        $Shortcut.WorkingDirectory = $InstallDir
        $Shortcut.WindowStyle = 0
        $Shortcut.Description = "TimeSplit - Automatic startup"
        $Shortcut.Save()
        
        Write-Success "Created startup shortcut at $ShortcutPath"
        return $true
    } catch {
        Write-Err "Failed to create startup shortcut: $_"
        return $false
    }
}

function Remove-StartupShortcut {
    if (Test-Path $ShortcutPath) {
        try {
            Remove-Item -Path $ShortcutPath -Force
            Write-Success "Removed startup shortcut"
        } catch {
            Write-Err "Failed to remove startup shortcut: $_"
        }
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

    # Add to PATH
    Add-ToPath -directory $InstallDir

    # Create startup shortcut
    Create-StartupShortcut -targetPath $InstallPath

    # Start the program immediately
    Write-Info "Starting $ProgramName..."
    try {
        Start-Process -FilePath $InstallPath -ArgumentList "run" -WindowStyle Hidden
        Write-Success "$ProgramName started successfully!"
    } catch {
        Write-Err "Failed to start ${ProgramName}: $_"
        Write-Info "You can start it manually with: $ProgramName run"
    }

    Write-Success "$ProgramName installed successfully!"
    Write-Info "The program is now running and will start automatically on next login."
    Write-Info "Use 'install.ps1 status' to check installation."
}

function Uninstall-Program {
    Ensure-Admin

    # Remove startup shortcut
    Write-Info "Removing startup shortcut (if present)"
    Remove-StartupShortcut

    # Remove from PATH
    Write-Info "Removing from system PATH"
    Remove-FromPath -directory $InstallDir

    # Remove installation directory
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
    Write-Host "`n=== Installation Status ===" -ForegroundColor Cyan
    
    # Check binary installation
    if (Test-Path $InstallPath) {
        Write-Success "$ProgramName is installed at $InstallPath"
        try {
            $version = & $InstallPath --version 2>$null
            if ($version) {
                Write-Info "Version: $version"
            }
        } catch { }
    } else {
        Write-Err "$ProgramName is not installed"
    }

    # Check PATH
    Write-Host "`n=== PATH Status ===" -ForegroundColor Cyan
    $currentPath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    if ($currentPath -split ';' | Where-Object { $_ -eq $InstallDir }) {
        Write-Success "$InstallDir is in system PATH"
    } else {
        Write-Err "$InstallDir is NOT in system PATH"
    }

    # Check startup shortcut
    Write-Host "`n=== Startup Status ===" -ForegroundColor Cyan
    if (Test-Path $ShortcutPath) {
        Write-Success "Startup shortcut exists at $ShortcutPath"
        try {
            $WScriptShell = New-Object -ComObject WScript.Shell
            $Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
            Write-Info "Target: $($Shortcut.TargetPath)"
            Write-Info "Arguments: $($Shortcut.Arguments)"
        } catch { }
    } else {
        Write-Err "Startup shortcut does NOT exist"
    }

    # Check if process is running
    Write-Host "`n=== Process Status ===" -ForegroundColor Cyan
    $process = Get-Process -Name $ProgramName -ErrorAction SilentlyContinue
    if ($process) {
        Write-Success "$ProgramName is currently running (PID: $($process.Id))"
    } else {
        Write-Info "$ProgramName is not currently running"
    }
    
    Write-Host ""
}

function Show-Usage {
    Write-Host "`nUsage: install.ps1 [command]`n"
    Write-Host "Commands:"
    Write-Host "  install    - Download and install $ProgramName"
    Write-Host "  uninstall  - Remove $ProgramName"
    Write-Host "  update     - Update to the latest version"
    Write-Host "  status     - Show installation status"
    Write-Host "  help       - Show this help message"
    Write-Host ""
}

switch ($Command.ToLower()) {
    'install' { Install-Program }
    'uninstall' { Uninstall-Program }
    'update' { Uninstall-Program; Install-Program }
    'status' { Show-Status }
    'help' { Show-Usage }
    default { Write-Err ("Invalid command: {0}`n" -f $Command); Show-Usage; exit 1 }
}