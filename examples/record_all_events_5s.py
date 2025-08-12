#!/usr/bin/env python3
"""
Show ALL events that are captured during recording, not just counts.
"""

import subprocess
import time
import os
import json
import re

def show_all_events():
    print("=" * 60)
    print("SHOWING ALL CAPTURED EVENTS")
    print("=" * 60)
    print("\nThis will show EVERY event captured during 5 seconds")
    print("-" * 60)
    
    # PowerShell command to run recorder
    ps_command = """
    $process = Start-Process -FilePath "cargo" -ArgumentList @("run", "--example", "debug_mcp_recording", "--release") -WorkingDirectory "../terminator-workflow-recorder" -NoNewWindow -PassThru -RedirectStandardOutput "all_events.txt" -RedirectStandardError "all_events_error.txt"
    Start-Sleep 5
    if (!$process.HasExited) { 
        $process.Kill()
    }
    """
    
    print("\n🔴 RECORDING - Click once and move mouse a bit\n")
    
    # Run the PowerShell command
    result = subprocess.run(
        ["powershell", "-Command", ps_command],
        capture_output=True,
        text=True,
        cwd="../terminator-workflow-recorder"
    )
    
    print("⏹️ Recording stopped\n")
    
    # Read the output file
    output_file = "../terminator-workflow-recorder/all_events.txt"
    if os.path.exists(output_file):
        with open(output_file, 'r', encoding='utf-8', errors='ignore') as f:
            lines = f.readlines()
        
        print("=" * 60)
        print("📊 ALL EVENTS CAPTURED (Raw Stream)")
        print("=" * 60)
        
        # Parse and show each event
        event_num = 0
        current_event = []
        events_list = []
        
        for line in lines:
            # Start of new event
            if "EVENT #" in line and "─────" in line:
                if current_event:
                    events_list.append(current_event)
                current_event = [line.strip()]
                event_num += 1
            elif event_num > 0 and "└─────" not in line:
                current_event.append(line.strip())
            elif "└─────" in line and current_event:
                current_event.append(line.strip())
                events_list.append(current_event)
                current_event = []
        
        # Add last event if exists
        if current_event:
            events_list.append(current_event)
        
        print(f"\n📈 Total Events Captured: {len(events_list)}")
        print("\n📝 Event Stream:\n")
        
        # Show each event
        for i, event_lines in enumerate(events_list, 1):
            print(f"EVENT {i}:")
            # Show first few lines of each event
            for line in event_lines[:8]:  # Show up to 8 lines per event
                if line:
                    print(f"  {line}")
            if len(event_lines) > 8:
                print(f"  ... ({len(event_lines) - 8} more lines)")
            print()
        
        # Analyze event types
        event_types = {
            "Click": 0,
            "Mouse": 0,
            "ApplicationSwitch": 0,
            "TextInput": 0,
            "Keyboard": 0,
            "Other": 0
        }
        
        mcp_conversions = 0
        unsupported = 0
        
        for event_lines in events_list:
            event_text = "\n".join(event_lines)
            
            # Count event types
            if "CLICK EVENT" in event_text or "Click event" in event_text:
                event_types["Click"] += 1
            elif "MOUSE EVENT" in event_text:
                event_types["Mouse"] += 1
            elif "APPLICATION SWITCH" in event_text:
                event_types["ApplicationSwitch"] += 1
            elif "TEXT INPUT" in event_text:
                event_types["TextInput"] += 1
            elif "KEYBOARD" in event_text:
                event_types["Keyboard"] += 1
            else:
                event_types["Other"] += 1
            
            # Count MCP conversions
            if "MCP CONVERSION" in event_text and "click_element" in event_text:
                mcp_conversions += 1
            if "unsupported" in event_text.lower() or "not implemented" in event_text:
                unsupported += 1
        
        print("\n" + "=" * 60)
        print("📊 EVENT TYPE SUMMARY")
        print("=" * 60)
        print("\nEvent Types Captured:")
        for event_type, count in event_types.items():
            if count > 0:
                print(f"  • {event_type}: {count}")
        
        print(f"\nMCP Conversions: {mcp_conversions}")
        print(f"Unsupported Events: {unsupported}")
        
        # Clean up
        os.remove(output_file)
        error_file = "../terminator-workflow-recorder/all_events_error.txt"
        if os.path.exists(error_file):
            os.remove(error_file)
            
    else:
        print("❌ No output file generated")
    
    print("\n" + "=" * 60)

if __name__ == "__main__":
    show_all_events()
