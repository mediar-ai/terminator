<#
  install-mcp-service.ps1
  Installs the MCP server as a Windows service that auto-starts at boot
  and restarts itself if it crashes.

  Usage example (run from an elevated PowerShell session):
  .\install-mcp-service.ps1 -ExecutablePath "C:\Program Files\MCP\mcp.exe"
#>

param(
    [Parameter(Mandatory=$true)]
    [string]$ExecutablePath,                 # Full path to mcp.exe

    [string]$Args        = "http",           # CLI args passed to mcp.exe
    [string]$ServiceName = "MCPServer",      # Internal service name
    [string]$DisplayName = "MCP Server",     # What shows up in Services MMC
    [string]$Description = "MCP server running in HTTP mode"
)

# Helper: quote a path that may contain spaces
function Quote($s) { return '"' + $s + '"' }

# If the service already exists, remove it first (optional safety-belt)
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    Write-Host "Service '$ServiceName' already exists – removing…" -ForegroundColor Yellow
    Stop-Service  -Name $ServiceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $ServiceName | Out-Null
    Start-Sleep 1
}

# Create the new service
$binPath = "$(Quote $ExecutablePath) $Args"
Write-Host "Creating service $ServiceName → $binPath"
New-Service -Name         $ServiceName `
            -BinaryPathName $binPath `
            -DisplayName   $DisplayName `
            -Description   $Description `
            -StartupType   Automatic

# Configure failure actions: restart after 5 s, up to 3 tries, never reset the counter
sc.exe failure     $ServiceName reset= 0 actions= restart/5000/restart/5000/restart/5000 | Out-Null
sc.exe failureflag $ServiceName 1                                                   | Out-Null

# Start it up!
Start-Service -Name $ServiceName
Write-Host "Service '$ServiceName' installed and started successfully." -ForegroundColor Green

<#
Uninstalling later:

Stop-Service MCPServer
sc.exe delete MCPServer
#>