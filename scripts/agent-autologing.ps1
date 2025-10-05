<#
.SYNOPSIS
    Auto-login Configuration Script with User Creation and Terminator MCP Agent Setup
.DESCRIPTION
    This script performs the following actions:
    - Creates a local user account (if it doesn't exist) and adds it to Administrators group
    - Configures Windows auto-login for the specified user
    - Configures Windows Firewall to allow TCP port 3000 in all profiles
    - Sets up Terminator MCP Agent to run automatically at login for ALL users
    - Disables diagnostic settings screen at login
    - Restarts the computer to apply changes
.USAGE
    Run as Administrator with default values:
    powershell -ExecutionPolicy Bypass -File .\agent-autologing.ps1

    Run with custom username and password:
    powershell -ExecutionPolicy Bypass -File .\agent-autologing.ps1 -Username "customuser" -Password "custompass"

    Run with domain:
    powershell -ExecutionPolicy Bypass -File .\agent-autologing.ps1 -Username "john" -Password "pass123" -Domain "MYDOMAIN"

    Run with all parameters:
    powershell -ExecutionPolicy Bypass -File .\agent-autologing.ps1 -Username "admin" -Password "SecurePass!123" -Domain "CORPORATE"
.NOTES
    Requires Administrator privileges
#>
# Run as Administrator
param(
    $Username = "mcp",
    $Password = "ai#25#AI#26#",
    $Domain = ""
)

# Check if user exists, create if not
$userExists = Get-LocalUser -Name $Username -ErrorAction SilentlyContinue
if (-not $userExists) {
    $securePassword = ConvertTo-SecureString $Password -AsPlainText -Force
    New-LocalUser -Name $Username -Password $securePassword -PasswordNeverExpires -AccountNeverExpires
    Add-LocalGroupMember -Group "Administrators" -Member $Username
    Write-Host "User '$Username' created and added to Administrators group"
} else {
    # Update password for existing user
    $securePassword = ConvertTo-SecureString $Password -AsPlainText -Force
    Set-LocalUser -Name $Username -Password $securePassword
    Write-Host "Password updated for existing user '$Username'"
}
# Configure auto-login
$path = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon"
Set-ItemProperty -Path $path -Name "AutoAdminLogon" -Value "1"
Set-ItemProperty -Path $path -Name "DefaultUserName" -Value $Username
Set-ItemProperty -Path $path -Name "DefaultPassword" -Value $Password
if ($Domain) { Set-ItemProperty -Path $path -Name "DefaultDomainName" -Value $Domain }
Write-Host "Auto-login configured for '$Username'"
# Configure firewall to allow TCP port 3000
Write-Host "`nConfiguring firewall for TCP port 3000..." -ForegroundColor Cyan
New-NetFirewallRule -DisplayName "Terminator MCP Agent" -Direction Inbound -Protocol TCP -LocalPort 3000 -Action Allow -Profile Any -ErrorAction SilentlyContinue
Write-Host "Firewall rule created for TCP port 3000"
# Get current IP address
$ipAddress = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notlike "*Loopback*" -and $_.IPAddress -notlike "169.254.*" } | Select-Object -First 1).IPAddress
Write-Host "Current IP address: $ipAddress" -ForegroundColor Yellow
# Add Terminator MCP Agent to autorun for ALL users
$exePath = Join-Path $env:ProgramFiles "terminator-mcp-agent.exe"
$startupCommand = "`"$exePath`" --transport http --port 3000 --host $ipAddress"
$runPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
Set-ItemProperty -Path $runPath -Name "TerminatorMCPAgent" -Value $startupCommand
Write-Host "`nTerminator MCP Agent added to autorun for ALL users"
Write-Host "Command: $startupCommand" -ForegroundColor Green

# Disable diagnostic settings screen and Server Manager at login
Write-Host "`nDisabling diagnostic settings and Server Manager at login..." -ForegroundColor Cyan

# Disable Server Manager at logon
$serverManagerPath = "HKLM:\SOFTWARE\Microsoft\ServerManager"
if (-not (Test-Path $serverManagerPath)) {
    New-Item -Path $serverManagerPath -Force | Out-Null
}
Set-ItemProperty -Path $serverManagerPath -Name "DoNotOpenServerManagerAtLogon" -Value 1 -Type DWord

# Disable Initial Configuration Tasks at logon
$oobePath = "HKLM:\SOFTWARE\Microsoft\ServerManager\Oobe"
if (-not (Test-Path $oobePath)) {
    New-Item -Path $oobePath -Force | Out-Null
}
Set-ItemProperty -Path $oobePath -Name "DoNotOpenInitialConfigurationTasksAtLogon" -Value 1 -Type DWord

# 1) Disable the privacy experience screen for all users
$OobeKey = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\OOBE'
New-Item -Path $OobeKey -Force | Out-Null
New-ItemProperty -Path $OobeKey -Name 'DisablePrivacyExperience' -Value 1 -PropertyType DWord -Force | Out-Null

# 2) (Optional) Force diagnostic data = Required only (aka Basic)
# Note: Value 0 (Security) is honored only on Enterprise/Education.
$DCKey = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection'
New-Item -Path $DCKey -Force | Out-Null
New-ItemProperty -Path $DCKey -Name 'AllowTelemetry' -Value 1 -PropertyType DWord -Force | Out-Null

# 3) (Optional) Reduce feedback nags
$SIKey = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Feedback'
New-Item -Path $SIKey -Force | Out-Null
New-ItemProperty -Path $SIKey -Name 'DoNotShowFeedbackNotifications' -Value 1 -PropertyType DWord -Force | Out-Null

Write-Host "Server Manager and diagnostic settings disabled at login" -ForegroundColor Green

Write-Host "`n✅ Setup completed successfully!" -ForegroundColor Green
Write-Host "`n🔄 Restarting computer in 5 seconds..." -ForegroundColor Yellow
Start-Sleep -Seconds 5
Restart-Computer -Force
