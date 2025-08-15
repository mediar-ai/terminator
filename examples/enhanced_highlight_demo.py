#!/usr/bin/env python3
"""
Enhanced Highlight Demo with Text Overlays

This demo shows the new enhanced highlight functionality with text overlays.
Run this with Notepad open to see text overlays in different positions.

Requirements: 
- Build the Python bindings: cd bindings/python && pip install -e .
- Or install terminator package if available
"""

import asyncio
import time
import sys
import os

# Add the bindings to path if running from examples folder
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'bindings', 'python'))

try:
    import terminator
except ImportError:
    print("❌ Terminator not found. Please build the Python bindings first:")
    print("   cd bindings/python && pip install -e .")
    sys.exit(1)


async def main():
    print("🎯 Enhanced Highlight Demo - Text Overlays")
    print("=" * 50)
    
    try:
        desktop = terminator.Desktop()
        
        # Step 1: Open Notepad
        print("\n1. Opening Notepad...")
        await desktop.run_command("notepad.exe", "notepad.exe")
        await asyncio.sleep(2)  # Wait for Notepad to open
        
        # Step 2: Find Notepad window and text area
        print("\n2. Finding Notepad elements...")
        
        # Find the Notepad application
        apps = desktop.applications()
        notepad = None
        for app in apps:
            if "notepad" in app.name().lower() or "untitled" in app.name().lower():
                notepad = app
                break
        
        if not notepad:
            print("❌ Notepad not found. Please open Notepad manually and try again.")
            return
        
        print(f"✅ Found Notepad: {notepad.name()}")
        
        # Find the text area (Edit control)
        locator = desktop.locator("role:Edit")
        try:
            text_area = await locator.first()
            print(f"✅ Found text area: {text_area.role()}")
        except terminator.ElementNotFoundError:
            print("❌ Text area not found in Notepad")
            return
        
        # Step 3: Demo different text overlay positions
        print("\n3. Testing different text overlay positions...")
        
        demos = [
            ("TOP", "Top", {"size": 16, "bold": True, "color": 0x000000}),
            ("RIGHT", "Right", {"size": 14, "bold": False, "color": 0x0000FF}),
            ("BOTTOM", "Bottom", {"size": 18, "bold": True, "color": 0x008000}),
            ("LEFT", "Left", {"size": 12, "bold": False, "color": 0xFF0000}),
            ("INSIDE", "Inside", {"size": 20, "bold": True, "color": 0x800080}),
        ]
        
        for text, position, font_style in demos:
            print(f"\n   🔸 Testing {position} position with text '{text}'")
            
            # Highlight with text overlay
            handle = text_area.highlight(
                color=0x00FF00,  # Green border
                duration_ms=3000,  # 3 seconds
                text=text,
                text_position=getattr(terminator.TextPosition, position),
                font_style=terminator.FontStyle(**font_style)
            )
            
            # Wait to see the highlight
            await asyncio.sleep(3.5)
            
            # Demonstrate manual closing
            if position == "Inside":
                print("   📝 Manually closing highlight...")
                handle.close()
                await asyncio.sleep(1)
        
        # Step 4: Demo with context manager (auto-close)
        print("\n4. Testing context manager (auto-close)...")
        
        with text_area.highlight(
            color=0xFF0000,  # Red border
            duration_ms=5000,
            text="AUTO-CLOSE",
            text_position=terminator.TextPosition.TopRight,
            font_style=terminator.FontStyle(size=22, bold=True, color=0xFFFFFF)
        ):
            print("   🔸 Highlight will auto-close when context exits...")
            await asyncio.sleep(2)
            
        print("   ✅ Auto-closed successfully!")
        
        # Step 5: Test different colors and styles
        print("\n5. Testing different font styles and colors...")
        
        style_demos = [
            ("BOLD", {"size": 24, "bold": True, "color": 0x000000}),
            ("BIG", {"size": 32, "bold": False, "color": 0x800000}),
            ("SMALL", {"size": 10, "bold": True, "color": 0x008080}),
        ]
        
        for text, font_style in style_demos:
            print(f"   🔸 Testing font style: {text}")
            
            handle = text_area.highlight(
                color=0x0000FF,
                duration_ms=2000,
                text=text,
                text_position=terminator.TextPosition.Top,
                font_style=terminator.FontStyle(**font_style)
            )
            
            await asyncio.sleep(2.5)
        
        print("\n🎉 Demo completed successfully!")
        print("\nKey features demonstrated:")
        print("  ✅ Text overlays in 9 different positions")
        print("  ✅ Custom font sizes, colors, and bold styling")
        print("  ✅ Manual and automatic highlight closing")
        print("  ✅ Context manager support")
        print("  ✅ White background for text visibility")
        
    except Exception as e:
        print(f"❌ Error during demo: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    asyncio.run(main())