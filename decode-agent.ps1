# Simple script to decode and start agent on VM
$script = @'
cd C:\agent
if (Test-Path a.b64) {
    $content = Get-Content a.b64 -Raw
    $bytes = [Convert]::FromBase64String($content)
    [IO.File]::WriteAllBytes("C:\agent\agent.exe", $bytes)
    "Agent decoded: $((Get-Item agent.exe).Length) bytes"
    Start-Process C:\agent\agent.exe -ArgumentList "--port 8080"
    Start-Sleep 3
    netstat -an | findstr 8080
} else {
    "No b64 file found"
}
'@

az vm run-command invoke -g REMOTE-UI-TEST-RG -n ui-test-vm --command-id RunPowerShellScript --scripts $script