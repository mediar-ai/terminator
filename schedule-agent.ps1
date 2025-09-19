Write-Host "Creating scheduled task to run agent ON AZURE VM..." -ForegroundColor Red

$task = @'
Unregister-ScheduledTask -TaskName "UIAgent" -Confirm:$false -ErrorAction SilentlyContinue
$action = New-ScheduledTaskAction -Execute "C:\agent\agent.exe" -Argument "--port 8080"
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddSeconds(5) -RepetitionInterval (New-TimeSpan -Minutes 5) -RepetitionDuration ([TimeSpan]::MaxValue)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
Register-ScheduledTask -TaskName "UIAgent" -Action $action -Trigger $trigger -Settings $settings -User "SYSTEM" -Force
Start-ScheduledTask -TaskName "UIAgent"
Start-Sleep 10
Get-ScheduledTask -TaskName "UIAgent" | Select State, TaskName
netstat -an | Select-String ":8080"
'@

az vm run-command invoke -g REMOTE-UI-TEST-RG -n ui-test-vm --command-id RunPowerShellScript --scripts $task