# Run all Python scripts in this directory except gnome* and macos* ones.

Set-Location -Path $PSScriptRoot

# Prefer the project venv's Python if available.
if (Test-Path ".venv\Scripts\python.exe") {
    $Python = ".venv\Scripts\python.exe"
} elseif (Test-Path ".venv\bin\python") {
    $Python = ".venv\bin\python"
} else {
    $Python = "python"
}

# Per-script command line arguments. Scripts not listed here run with no args.
# The URL is kept as a single quoted string so the '&' characters are passed
# through literally instead of being treated as PowerShell operators.
$ScriptArgs = @{
    "vlc_auto_player.py" = @(
        "--youtube-link",
        "https://www.youtube.com/watch?v=YQHsXMglC9A&list=RDYQHsXMglC9A&start_radio=1"
    )
}

Get-ChildItem -Path . -Filter *.py |
    # Skip gnome*/macos* (other-OS demos) and _*-prefixed scratch/diagnostic
    # scripts like _diag_vlc.py, which also opens VLC and would otherwise launch
    # a second VLC window at the start of the run.
    Where-Object { $_.Name -notlike "gnome*" -and $_.Name -notlike "macos*" -and $_.Name -notlike "_*" } |
    # Keep the default alphabetical order, but force vlc_auto_player.py to run
    # last (it opens VLC and streams for a while, so it's nicest at the end).
    Sort-Object @{ Expression = { $_.Name -eq "vlc_auto_player.py" } }, Name |
    ForEach-Object {
        $extraArgs = $ScriptArgs[$_.Name]
        Write-Host "=== Running $($_.Name) ==="
        & $Python $_.Name @extraArgs
        Write-Host "=== Finished $($_.Name) (exit $LASTEXITCODE) ==="
        Write-Host ""
    }
