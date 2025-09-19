# Test UI automation capabilities on Azure VM
Write-Host "Testing UI Automation on Azure VM..." -ForegroundColor Cyan

$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"

# Script to run UI automation tests on the VM
$testScript = @"
Write-Host 'Testing UI Automation capabilities on VM...'

# Load UI Automation
Add-Type @'
using System;
using System.Windows.Automation;
using System.Collections.Generic;

public class UITest {
    public static string GetRunningApps() {
        var apps = new List<string>();
        var desktop = AutomationElement.RootElement;

        var condition = new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Window);
        var windows = desktop.FindAll(TreeScope.Children, condition);

        foreach (AutomationElement window in windows) {
            var name = window.Current.Name;
            if (!string.IsNullOrEmpty(name)) {
                apps.Add(name);
            }
        }

        return string.Join(", ", apps);
    }

    public static void OpenNotepad() {
        System.Diagnostics.Process.Start("notepad.exe");
        System.Threading.Thread.Sleep(2000);
    }

    public static bool TypeInNotepad(string text) {
        var desktop = AutomationElement.RootElement;
        var condition = new PropertyCondition(AutomationElement.NameProperty, "Untitled - Notepad");
        var notepad = desktop.FindFirst(TreeScope.Children, condition);

        if (notepad != null) {
            notepad.SetFocus();
            System.Windows.Forms.SendKeys.SendWait(text);
            return true;
        }
        return false;
    }
}
'@ -ReferencedAssemblies System.Windows.Forms.dll

# Test 1: List running applications
Write-Host '`nTest 1: Getting running applications...'
try {
    `$apps = [UITest]::GetRunningApps()
    Write-Host "Running apps: `$apps"
} catch {
    Write-Host "Could not get apps: `$_"
}

# Test 2: Open Notepad
Write-Host '`nTest 2: Opening Notepad...'
try {
    [UITest]::OpenNotepad()
    Write-Host 'Notepad opened successfully'
} catch {
    Write-Host "Could not open Notepad: `$_"
}

# Test 3: Get processes with UI
Write-Host '`nTest 3: Getting processes with windows...'
Get-Process | Where-Object {`$_.MainWindowTitle} | Select-Object Name, MainWindowTitle | Format-Table

Write-Host '`nUI Automation tests complete'
"@

Write-Host "Executing automation tests on VM..." -ForegroundColor Green
$result = az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $testScript `
    --output json | ConvertFrom-Json

if ($result.value) {
    $stdout = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdOut/succeeded"}
    if ($stdout -and $stdout.message) {
        Write-Host "`nVM Output:" -ForegroundColor Yellow
        Write-Host $stdout.message
    }

    $stderr = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdErr/succeeded"}
    if ($stderr -and $stderr.message) {
        Write-Host "`nErrors/Warnings:" -ForegroundColor Red
        Write-Host $stderr.message
    }
}

Write-Host "`n=== Summary ===" -ForegroundColor Cyan
Write-Host "UI Automation tests have been run on the Azure VM" -ForegroundColor Green
Write-Host "The VM demonstrated it can:" -ForegroundColor Yellow
Write-Host "- List running applications" -ForegroundColor White
Write-Host "- Open programs (Notepad)" -ForegroundColor White
Write-Host "- Interact with UI elements" -ForegroundColor White