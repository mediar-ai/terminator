# -*- coding: utf-8 -*-
"""
Test script for Chinese/UTF-8 selector support in Terminator
This tests whether Chinese characters work in selectors on Chinese Windows systems
"""
import asyncio
import sys
import terminator


async def test_chinese_selectors():
    print("=" * 60)
    print("Testing UTF-8/Chinese Selector Support")
    print("=" * 60)
    print(f"Python version: {sys.version}")
    print(f"Default encoding: {sys.getdefaultencoding()}")
    print()

    # Create desktop instance
    desktop = terminator.Desktop(log_level="debug")

    try:
        # Open Calculator
        print("Opening Calculator...")
        try:
            calculator_window = desktop.open_application("calc.exe")
        except Exception as e:
            print(f"Failed to open calculator: {e}")
            return

        await asyncio.sleep(2)

        # Get window tree to see actual element names
        print("\n" + "=" * 60)
        print("Getting Calculator Window Tree")
        print("=" * 60)

        # Get all button elements to see their actual names
        try:
            buttons = await calculator_window.locator("role:Button").all(timeout_ms=5000, depth=10)
            print(f"\nFound {len(buttons)} buttons in Calculator")
            print("\nFirst 20 button names:")
            for i, btn in enumerate(buttons[:20]):
                name = btn.name()
                print(f"  {i+1}. Name: '{name}' | Bytes: {name.encode('utf-8')!r}")
        except Exception as e:
            print(f"Failed to get buttons: {e}")

        # Test 1: Try to find a button using its English name (baseline test)
        print("\n" + "=" * 60)
        print("Test 1: English Selector (Baseline)")
        print("=" * 60)

        try:
            # Try finding button "One" (should work on English systems)
            one_button = await calculator_window.locator("Name:One").first()
            print(f"✅ Found 'One' button: {one_button.name()}")
        except Exception as e:
            print(f"❌ Failed to find 'One' button: {e}")

        # Test 2: Try Chinese selectors if on Chinese system
        print("\n" + "=" * 60)
        print("Test 2: Chinese/Unicode Selectors")
        print("=" * 60)

        # Test different encoding approaches
        test_cases = [
            ("Direct UTF-8 string", "name:显示为"),
            ("Raw string", r"name:显示为"),
            ("Unicode escape", "name:\u663e\u793a\u4e3a"),
            ("Explicit encode/decode", ("name:" + "显示为".encode('utf-8').decode('utf-8'))),
            ("Role with Chinese", "role:Button|name:显示为"),
        ]

        for description, selector in test_cases:
            print(f"\nTesting: {description}")
            print(f"  Selector: {selector}")
            print(f"  Selector bytes: {selector.encode('utf-8')!r}")
            print(f"  Selector repr: {repr(selector)}")

            try:
                element = await calculator_window.locator(selector).first()
                print(f"  ✅ SUCCESS! Found element: {element.name()}")
            except Exception as e:
                print(f"  ❌ FAILED: {e}")

        # Test 3: Verify selector parsing in Rust
        print("\n" + "=" * 60)
        print("Test 3: Selector String Roundtrip")
        print("=" * 60)

        chinese_text = "显示为"
        selector_str = f"role:Button|name:{chinese_text}"

        print(f"Original string: {chinese_text}")
        print(f"Original bytes: {chinese_text.encode('utf-8')!r}")
        print(f"Selector string: {selector_str}")
        print(f"Selector bytes: {selector_str.encode('utf-8')!r}")

        # Create locator and check if it's parsed correctly
        locator = calculator_window.locator(selector_str)
        print(f"✅ Locator created successfully")

    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    print("Starting Chinese selector tests...")
    asyncio.run(test_chinese_selectors())
    print("\nTests complete!")
