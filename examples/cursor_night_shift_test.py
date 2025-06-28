#!/usr/bin/env python3
"""
Cursor Night Shift Agent - Quick Test Script

This script performs a quick test to verify that:
1. Terminator can find Cursor
2. Basic automation works
3. Chat interface can be accessed

Run this before using the full night shift agent to verify your setup.
"""

import asyncio
import terminator
import logging

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

async def test_cursor_automation():
    """
    Test basic Cursor automation functionality.
    """
    print("🧪 Cursor Night Shift Agent - Quick Test")
    print("=" * 40)
    
    desktop = terminator.Desktop(log_level="info")
    
    try:
        # Test 1: Find Cursor window
        print("\n1️⃣ Testing: Find Cursor window...")
        cursor_selectors = [
            "name:Cursor",
            "name:cursor",
            "window:Cursor", 
            "window:cursor"
        ]
        
        cursor_app = None
        for selector in cursor_selectors:
            try:
                cursor_window = await desktop.locator(selector).first()
                if cursor_window:
                    print(f"✅ Found Cursor using selector: {selector}")
                    cursor_window.highlight(color=0x00FF00, duration_ms=2000)  # Green highlight
                    await cursor_window.focus()
                    cursor_app = cursor_window
                    await asyncio.sleep(2)
                    break
            except Exception as e:
                print(f"❌ Selector {selector} failed: {e}")
                continue
        
        if not cursor_app:
            print("❌ Could not find Cursor window. Please ensure Cursor is running.")
            return False
            
        # Test 2: Find chat input
        print("\n2️⃣ Testing: Find chat input...")
        chat_selectors = [
            "role:textbox",
            "role:EditableText",
            "role:Edit"
        ]
        
        chat_input = None
        for selector in chat_selectors:
            try:
                found_input = await cursor_app.locator(selector).first()
                if found_input and await found_input.is_visible():
                    print(f"✅ Found chat input using selector: {selector}")
                    found_input.highlight(color=0x0000FF, duration_ms=2000)  # Blue highlight
                    chat_input = found_input
                    break
            except Exception as e:
                print(f"❌ Chat selector {selector} failed: {e}")
                continue
        
        # If no chat input found, try keyboard shortcuts
        if not chat_input:
            print("⚠️ Chat input not immediately visible, trying keyboard shortcuts...")
            shortcuts = ["{Ctrl}l", "{Ctrl}k"]
            
            for shortcut in shortcuts:
                try:
                    print(f"Trying shortcut: {shortcut}")
                    await cursor_app.press_key(shortcut)
                    await asyncio.sleep(2)
                    
                    # Check if chat input appeared
                    for selector in chat_selectors:
                        try:
                            found_input = await cursor_app.locator(selector).first()
                            if found_input and await found_input.is_visible():
                                print(f"✅ Chat opened with shortcut {shortcut}, found input: {selector}")
                                found_input.highlight(color=0x0000FF, duration_ms=2000)
                                chat_input = found_input
                                break
                        except:
                            continue
                    
                    if chat_input:
                        break
                        
                except Exception as e:
                    print(f"❌ Shortcut {shortcut} failed: {e}")
        
        # Test 3: Send a test message
        if chat_input:
            print("\n3️⃣ Testing: Send test prompt...")
            test_prompt = "Hello! This is a test from the Cursor Night Shift Agent. Please respond with 'Test received' to confirm."
            
            try:
                await chat_input.focus()
                await asyncio.sleep(0.5)
                
                # Clear any existing text
                await chat_input.press_key("{Ctrl}a")
                await asyncio.sleep(0.2)
                
                # Type test prompt
                await chat_input.type_text(test_prompt)
                await asyncio.sleep(1)
                
                print("✅ Successfully typed test prompt")
                print("📤 Test prompt ready to send (NOT SENDING automatically)")
                print("👉 You can manually press Enter to send if you want to test the full flow")
                
                # Highlight the text that was typed
                chat_input.highlight(color=0xFFFF00, duration_ms=3000)  # Yellow highlight
                
            except Exception as e:
                print(f"❌ Failed to type test prompt: {e}")
                return False
        else:
            print("❌ Could not find or access chat input")
            return False
            
        print("\n🎉 Test completed successfully!")
        print("\nTest Summary:")
        print("✅ Cursor window found and focused")
        print("✅ Chat input located and accessible") 
        print("✅ Text input working")
        print("\n🚀 Your setup is ready for the Night Shift Agent!")
        print("\nNext steps:")
        print("1. Review and customize the prompts in cursor_night_shift_agent.py")
        print("2. Adjust the INTERVAL_SECONDS if needed")
        print("3. Run: python cursor_night_shift_agent.py")
        
        return True
        
    except Exception as e:
        print(f"💥 Unexpected error during test: {e}")
        return False

async def main():
    """
    Main test function.
    """
    print("🌙 Testing Cursor automation setup...")
    print("⏰ This test will take about 10-15 seconds")
    print("👀 Watch for colored highlights on Cursor UI elements\n")
    
    success = await test_cursor_automation()
    
    if success:
        print("\n✅ All tests passed! You're ready to use the Night Shift Agent.")
    else:
        print("\n❌ Some tests failed. Please check the issues above and try again.")
        print("\nTroubleshooting tips:")
        print("- Make sure Cursor is running and visible")
        print("- Try opening Cursor's chat manually (Ctrl+L)")
        print("- Check that Cursor is not in full-screen mode")
        print("- Ensure Cursor has focus and is the active window")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n🛑 Test interrupted by user")
    except Exception as e:
        print(f"\n💥 Test failed with error: {e}")