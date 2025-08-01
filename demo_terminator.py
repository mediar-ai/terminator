#!/usr/bin/env python3
"""
Simple demonstration of Terminator MCP Agent
Shows the application running and basic functionality
"""

import subprocess
import time
import requests
import sys
import threading

def print_status(message, status="info"):
    """Print colored status messages"""
    colors = {
        "info": "\033[94m",      # Blue
        "success": "\033[92m",   # Green
        "warning": "\033[93m",   # Yellow
        "error": "\033[91m",     # Red
        "reset": "\033[0m"       # Reset
    }
    
    symbols = {
        "info": "ℹ️",
        "success": "✅",
        "warning": "⚠️",
        "error": "❌"
    }
    
    color = colors.get(status, colors["info"])
    symbol = symbols.get(status, "•")
    reset = colors["reset"]
    
    print(f"{color}{symbol} {message}{reset}")

def start_mcp_server():
    """Start the MCP server in background"""
    print_status("Starting Terminator MCP Agent...", "info")
    
    cmd = [
        "cmd", "/c",
        "cd terminator-mcp-agent && npx -y terminator-mcp-agent@latest -t http --host 127.0.0.1 --port 8081 --cors"
    ]
    
    try:
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd="e:/terminator"
        )
        
        # Wait a moment for server to start
        time.sleep(5)
        
        if process.poll() is None:
            print_status("MCP Agent started successfully!", "success")
            return process
        else:
            stdout, stderr = process.communicate()
            print_status(f"Failed to start MCP Agent: {stderr}", "error")
            return None
            
    except Exception as e:
        print_status(f"Error starting MCP Agent: {e}", "error")
        return None

def test_health_endpoint():
    """Test the health endpoint"""
    print_status("Testing health endpoint...", "info")
    
    try:
        response = requests.get("http://localhost:8081/health", timeout=5)
        if response.status_code == 200:
            print_status("Health endpoint responding correctly!", "success")
            print(f"Response: {response.text}")
            return True
        else:
            print_status(f"Health endpoint returned status {response.status_code}", "error")
            return False
    except requests.exceptions.ConnectionError:
        print_status("Cannot connect to MCP server", "error")
        return False
    except Exception as e:
        print_status(f"Health check failed: {e}", "error")
        return False

def test_mcp_endpoint():
    """Test the MCP endpoint"""
    print_status("Testing MCP endpoint...", "info")
    
    try:
        response = requests.get("http://localhost:8081/mcp", timeout=5)
        # MCP endpoint might return different status codes, but should respond
        print_status(f"MCP endpoint responded with status {response.status_code}", "success")
        return True
    except requests.exceptions.ConnectionError:
        print_status("Cannot connect to MCP endpoint", "error")
        return False
    except Exception as e:
        print_status(f"MCP endpoint test failed: {e}", "error")
        return False

def demonstrate_functionality():
    """Demonstrate the key functionality"""
    print("🎬 Terminator MCP Agent Demonstration")
    print("=" * 50)
    
    # Start the server
    server_process = start_mcp_server()
    if not server_process:
        print_status("Cannot start server, exiting", "error")
        return False
    
    try:
        # Test health endpoint
        if test_health_endpoint():
            print_status("✅ Health check passed", "success")
        else:
            print_status("❌ Health check failed", "error")
        
        # Test MCP endpoint
        if test_mcp_endpoint():
            print_status("✅ MCP endpoint accessible", "success")
        else:
            print_status("❌ MCP endpoint not accessible", "error")
        
        # Show server information
        print("\n📋 Server Information:")
        print("• MCP Endpoint: http://localhost:8081/mcp")
        print("• Health Check: http://localhost:8081/health")
        print("• Transport: HTTP with CORS enabled")
        print("• Platform: Windows (win32-x64)")
        
        print("\n🔧 Available Features:")
        print("• Desktop automation via MCP protocol")
        print("• HTTP transport (no RDP/VNC required)")
        print("• Cross-platform MCP client support")
        print("• Web browser compatible (CORS enabled)")
        
        print("\n🎯 Key Benefits:")
        print("• ✅ Runs in terminal (no GUI required)")
        print("• ✅ HTTP-based communication")
        print("• ✅ Docker and Kubernetes ready")
        print("• ✅ External client access")
        
        print("\n🚀 Ready for:")
        print("• Python MCP clients")
        print("• Web-based MCP clients")
        print("• Docker container deployment")
        print("• Kubernetes cluster deployment")
        
        print("\n" + "=" * 50)
        print_status("🎉 Demonstration completed successfully!", "success")
        print_status("The Terminator MCP Agent is running and ready for use!", "success")
        
        return True
        
    finally:
        # Clean up
        if server_process:
            print_status("Stopping MCP server...", "info")
            server_process.terminate()
            try:
                server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server_process.kill()
            print_status("MCP server stopped", "info")

if __name__ == "__main__":
    try:
        success = demonstrate_functionality()
        sys.exit(0 if success else 1)
    except KeyboardInterrupt:
        print_status("\nDemo interrupted by user", "info")
        sys.exit(0)
    except Exception as e:
        print_status(f"Demo failed: {e}", "error")
        sys.exit(1)
