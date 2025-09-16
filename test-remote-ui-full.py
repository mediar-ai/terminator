#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Full test of remote UI automation features
Tests all terminator capabilities through REST API
"""

import requests
import json
import time
import base64

AGENT_URL = "http://localhost:8085"

def test_health():
    """Test 1: Health check"""
    print("\n1. Testing Health Check...")
    response = requests.get(f"{AGENT_URL}/health")
    data = response.json()
    print(f"   [OK] Status: {data['status']}")
    print(f"   [OK] Service: {data['service']}")
    return response.status_code == 200

def test_get_applications():
    """Test 2: List running applications"""
    print("\n2. Testing GetApplications...")
    payload = {
        "action": {"type": "GetApplications"},
        "request_id": "test-apps"
    }
    response = requests.post(f"{AGENT_URL}/execute", json=payload)
    data = response.json()

    if data['success']:
        apps = data['data']
        print(f"   [OK] Found {len(apps)} applications")
        for app in apps[:5]:  # Show first 5
            name = app['name'].encode('ascii', 'ignore').decode('ascii')  # Remove non-ASCII
            print(f"     - {name} (PID: {app['process_id']})")
        return True
    return False

def open_notepad():
    """Test 3: Open Notepad"""
    print("\n3. Opening Notepad...")
    import subprocess
    subprocess.Popen("notepad.exe")
    time.sleep(2)
    print("   [OK] Notepad opened")
    return True

def test_type_text():
    """Test 4: Type text in Notepad"""
    print("\n4. Testing TypeText in Notepad...")

    # First find Notepad window
    payload = {
        "action": {
            "type": "TypeText",
            "selector": "role:Edit",  # Target the edit control in Notepad
            "text": "Hello from Remote UI Automation!\nThis is terminator working remotely.\n"
        },
        "request_id": "test-type"
    }

    try:
        response = requests.post(f"{AGENT_URL}/execute", json=payload)
        data = response.json()
        if data.get('success'):
            print("   [OK] Text typed successfully!")
            return True
        else:
            print(f"   [WARN] TypeText needs exact selector: {data.get('error')}")
    except Exception as e:
        print(f"   [WARN] Error: {e}")
    return False

def open_calculator():
    """Test 5: Open Calculator"""
    print("\n5. Opening Calculator...")
    import subprocess
    subprocess.Popen("calc.exe")
    time.sleep(2)
    print("   [OK] Calculator opened")
    return True

def test_click_calculator():
    """Test 6: Click calculator buttons"""
    print("\n6. Testing Click on Calculator...")

    # Click button "7"
    payload = {
        "action": {
            "type": "Click",
            "selector": "role:Button|name:Seven"
        },
        "request_id": "test-click-7"
    }

    try:
        response = requests.post(f"{AGENT_URL}/execute", json=payload)
        data = response.json()
        if data.get('success'):
            print("   [OK] Clicked button 7")
            time.sleep(0.5)

            # Click button "+"
            payload['action']['selector'] = "role:Button|name:Plus"
            payload['request_id'] = "test-click-plus"
            response = requests.post(f"{AGENT_URL}/execute", json=payload)
            if response.json().get('success'):
                print("   [OK] Clicked button +")
                time.sleep(0.5)

                # Click button "3"
                payload['action']['selector'] = "role:Button|name:Three"
                payload['request_id'] = "test-click-3"
                response = requests.post(f"{AGENT_URL}/execute", json=payload)
                if response.json().get('success'):
                    print("   [OK] Clicked button 3")
                    print("   [OK] Calculator automation successful (7 + 3)")
                    return True
    except Exception as e:
        print(f"   [WARN] Calculator buttons may have different names: {e}")
    return False

def test_wait_for_element():
    """Test 7: Wait for element"""
    print("\n7. Testing WaitForElement...")

    payload = {
        "action": {
            "type": "WaitForElement",
            "selector": "role:Window|name:Calculator",
            "condition": "visible",
            "timeout_ms": 2000
        },
        "request_id": "test-wait"
    }

    try:
        response = requests.post(f"{AGENT_URL}/execute", json=payload)
        data = response.json()
        if data.get('success'):
            print("   [OK] Element found and visible")
            return True
        else:
            print(f"   [WARN] {data.get('error')}")
    except Exception as e:
        print(f"   [WARN] Error: {e}")
    return False

def test_element_properties():
    """Test 8: Get element properties"""
    print("\n8. Testing GetElementProperties...")

    payload = {
        "action": {
            "type": "GetElementProperties",
            "selector": "role:Window|name:Calculator"
        },
        "request_id": "test-props"
    }

    try:
        response = requests.post(f"{AGENT_URL}/execute", json=payload)
        data = response.json()
        if data.get('success'):
            props = data['data']
            print(f"   [OK] Got properties for: {props.get('name', 'element')}")
            print(f"     - Role: {props.get('role')}")
            print(f"     - Visible: {props.get('is_visible')}")
            print(f"     - Enabled: {props.get('is_enabled')}")
            return True
    except Exception as e:
        print(f"   [WARN] Error: {e}")
    return False

def test_screenshot():
    """Test 9: Take screenshot"""
    print("\n9. Testing Screenshot...")

    payload = {
        "action": {
            "type": "TakeScreenshot",
            "selector": "role:Window|name:Calculator"
        },
        "request_id": "test-screenshot"
    }

    try:
        response = requests.post(f"{AGENT_URL}/execute", json=payload)
        data = response.json()
        if data.get('success'):
            screenshot_b64 = data['data'].get('screenshot')
            if screenshot_b64:
                # Save screenshot
                screenshot_data = base64.b64decode(screenshot_b64)
                with open("test-screenshot.png", "wb") as f:
                    f.write(screenshot_data)
                print("   [OK] Screenshot captured and saved as test-screenshot.png")
                return True
            else:
                print("   [WARN] Screenshot data empty")
    except Exception as e:
        print(f"   [WARN] Screenshot feature: {e}")
    return False

def cleanup():
    """Clean up test windows"""
    print("\n10. Cleaning up...")
    import subprocess
    subprocess.run(["taskkill", "/F", "/IM", "notepad.exe"], capture_output=True)
    subprocess.run(["taskkill", "/F", "/IM", "CalculatorApp.exe"], capture_output=True)
    subprocess.run(["taskkill", "/F", "/IM", "calc.exe"], capture_output=True)
    print("   [OK] Cleaned up test windows")

def main():
    print("="*50)
    print("REMOTE UI AUTOMATION TEST SUITE")
    print("Testing all terminator features via REST API")
    print("="*50)

    results = []

    # Run all tests
    results.append(("Health Check", test_health()))
    results.append(("Get Applications", test_get_applications()))
    results.append(("Open Notepad", open_notepad()))
    results.append(("Type Text", test_type_text()))
    results.append(("Open Calculator", open_calculator()))
    results.append(("Click Buttons", test_click_calculator()))
    results.append(("Wait for Element", test_wait_for_element()))
    results.append(("Element Properties", test_element_properties()))
    results.append(("Screenshot", test_screenshot()))

    # Cleanup
    time.sleep(2)
    cleanup()

    # Print summary
    print("\n" + "="*50)
    print("TEST RESULTS SUMMARY")
    print("="*50)

    passed = sum(1 for _, result in results if result)
    total = len(results)

    for test_name, result in results:
        status = "[PASS]" if result else "[FAIL]"
        print(f"{status} - {test_name}")

    print(f"\nTotal: {passed}/{total} tests passed")

    if passed == total:
        print("\n*** ALL TESTS PASSED! Remote UI Automation is fully functional! ***")
    else:
        print(f"\n[WARNING] {total - passed} tests need attention")

if __name__ == "__main__":
    main()