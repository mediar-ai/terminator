Get-WmiObject Win32_Process | Where-Object { $_.ExecutablePath -like '*terminator-mcp-agent*' } | ForEach-Object {
  taskkill.exe /T /F /PID $_.ProcessId *>$null
}
Write-Host "installing Terminator MCP..."

try {
    # get latest version
    $releases = Invoke-RestMethod "https://api.github.com/repos/mediar-ai/terminator/releases"
    $latestRelease = $releases | Where-Object { -not $_.prerelease } | Select-Object -First 1
    if (-not $latestRelease) {
        throw "no releases found"
    }

    # find the Windows asset
    $asset = $latestRelease.assets | Where-Object { $_.name -like "terminator-mcp-windows-x86_64.zip" } | Select-Object -First 1
    if (-not $asset) {
        throw "no Terminator MCP release found in version $($latestRelease.tag_name)"
    }

    $url = $asset.browser_download_url

    $installDir = "$env:USERPROFILE\.terminator"
    $tempZip = "$env:TEMP\terminator-mcp-windows-x86_64.zip"

    # download and extract
    Write-Host "downloading latest version ($($latestRelease.tag_name)) from $url..."
    Invoke-WebRequest -Uri $url -OutFile $tempZip

    # create install directory if it doesn't exist
    if (!(Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir | Out-Null
    }

    Expand-Archive -Path $tempZip -DestinationPath $installDir -Force

    # add to PATH if not already there
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$installDir\*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$installDir\", "User")
        $env:Path = [Environment]::GetEnvironmentVariable("Path", "User")
    }

    # verify installation
    $binPath = Join-Path $installDir "terminator-mcp-agent.exe"
    if (!(Test-Path $binPath)) {
        throw "terminator-mcp-agent.exe not found in $binPath after installation"
    }

    Write-Host "Terminator MCP added to the PATH"

    Write-Host "Initializing Terminator MCP setup for desktop apps"
    Write-Host "Which app do you want to configure Terminator MCP for?"
    Write-Host "1. Cursor"
    Write-Host "2. Claude"
    $choice = Read-Host "Enter your choice (1 or 2)"

    switch ($choice) {
        1 {
            # configure for Cursor
            $cursorConfigDir = "$env:USERPROFILE\.cursor"
            if (!(Test-Path $cursorConfigDir)) {
                New-Item -ItemType Directory -Path $cursorConfigDir | Out-Null
            }
            $cursorConfigFile = Join-Path $cursorConfigDir "mcp.json"
            if (-not (Test-Path $cursorConfigFile)) {
                New-Item -ItemType File -Path $cursorConfigFile -Force
            } 
            $cursorConfigContent = Get-Content -Raw -Path $cursorConfigFile | ConvertFrom-Json
            if (-not $cursorConfigContent -or $cursorConfigContent.PSObject.Properties.Value.Count -eq 0) {
                $cursorConfigContent = [PSCustomObject]@{
                    mcpServers = @{}
                }
            }
            if (-not $cursorConfigContent.mcpServers) {
                $cursorConfigContent.mcpServers = @{}
            } elseif (-not ($cursorConfigContent.mcpServers -is [hashtable])) {
                $tempHashtable = @{}
                foreach ($property in $cursorConfigContent.mcpServers.PSObject.Properties) {
                    $tempHashtable[$property.Name] = $property.Value
                }
                $cursorConfigContent.mcpServers = $tempHashtable
            }
            # add the terminator-mcp-agent key
            $cursorConfigContent.mcpServers["terminator-mcp-agent"] = @{
                command = "terminator-mcp-agent.exe"
                args = @()
            }

            # might be cluttered json :(
            $formattedJson = $cursorConfigContent | ConvertTo-Json -Depth 10 -Compress:$true
            Set-Content -Path $cursorConfigFile -Value $formattedJson
            Write-Host "Cursor configuration saved to $cursorConfigFile"
        }
        2 {
            # Configure for Claude
            $claudeConfigFile = "$env:APPDATA\Claude\claude_desktop_config.json"
            if (!(Test-Path (Split-Path $claudeConfigFile))) {
                throw "You've likely not installed the Claude desktop app, please install it!!"
            }

            $claudeConfigContent = Get-Content -Raw -Path $claudeConfigFile | ConvertFrom-Json
            if (-not $claudeConfigContent -or $claudeConfigContent.PSObject.Properties.Value.Count -eq 0) {
                $claudeConfigContent = [PSCustomObject]@{
                    mcpServers = @{}
                }
            }
            if (-not $claudeConfigContent.mcpServers) {
                $claudeConfigContent.mcpServers = @{}
            } elseif (-not ($claudeConfigContent.mcpServers -is [hashtable])) {
                $tempHashtable = @{}
                foreach ($property in $claudeConfigContent.mcpServers.PSObject.Properties) {
                    $tempHashtable[$property.Name] = $property.Value
                }
                $claudeConfigContent.mcpServers = $tempHashtable
            }

            # add the terminator-mcp-agent key
            $claudeConfigContent.mcpServers["terminator-mcp-agent"] = @{
                command = "terminator-mcp-agent.exe"
                args = @()
            }

            $formattedJson = $claudeConfigContent | ConvertTo-Json -Depth 10 -Compress:$true
            Set-Content -Path $claudeConfigFile -Value $formattedJson
            Write-Host "Claude configuration saved to $claudeConfigFile"
        }
        Default {
            Write-Host "Invalid choice. Skipping app configuration."
        }
    }

    # cleanup
    Remove-Item $tempZip -Force
    Write-Host "terminator-mcp-agent installed successfully, restart the app"
} catch {
    Write-Host "installation failed: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
