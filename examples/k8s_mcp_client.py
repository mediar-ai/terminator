#!/usr/bin/env python3
"""
Kubernetes MCP Client for Terminator Desktop Automation
Connects to Terminator MCP Agent running in Kubernetes via HTTP

Prerequisites:
pip install mcp kubernetes requests

Usage:
python k8s_mcp_client.py --namespace terminator --service terminator-mcp-service
"""

import asyncio
import argparse
import json
import subprocess
import threading
import time
from typing import Optional, Dict, Any
from contextlib import AsyncExitStack

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client
from kubernetes import client, config
from kubernetes.client.rest import ApiException


class KubernetesMCPClient:
    def __init__(self, namespace: str = "terminator", service_name: str = "terminator-mcp-service", port: int = 8080):
        self.namespace = namespace
        self.service_name = service_name
        self.port = port
        self.local_port = 8080
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        self.port_forward_process = None
        
        # Initialize Kubernetes client
        try:
            config.load_incluster_config()  # Try in-cluster config first
        except:
            try:
                config.load_kube_config()  # Fall back to local kubeconfig
            except Exception as e:
                print(f"❌ Failed to load Kubernetes config: {e}")
                raise
        
        self.v1 = client.CoreV1Api()
    
    def print_status(self, message: str, status: str = "info"):
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
    
    def check_service_exists(self) -> bool:
        """Check if the MCP service exists in Kubernetes"""
        try:
            service = self.v1.read_namespaced_service(
                name=self.service_name,
                namespace=self.namespace
            )
            self.print_status(f"Found service '{self.service_name}' in namespace '{self.namespace}'", "success")
            return True
        except ApiException as e:
            if e.status == 404:
                self.print_status(f"Service '{self.service_name}' not found in namespace '{self.namespace}'", "error")
            else:
                self.print_status(f"Error checking service: {e}", "error")
            return False
    
    def start_port_forward(self) -> bool:
        """Start kubectl port-forward to the service"""
        self.print_status(f"Starting port-forward to {self.service_name}:{self.port}...", "info")
        
        cmd = [
            "kubectl", "port-forward", "-n", self.namespace,
            f"svc/{self.service_name}", f"{self.local_port}:{self.port}"
        ]
        
        try:
            # Start port-forward in background
            self.port_forward_process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            # Wait a moment for port-forward to establish
            time.sleep(3)
            
            # Check if process is still running
            if self.port_forward_process.poll() is None:
                self.print_status(f"Port-forward established on localhost:{self.local_port}", "success")
                return True
            else:
                stdout, stderr = self.port_forward_process.communicate()
                self.print_status(f"Port-forward failed: {stderr}", "error")
                return False
                
        except Exception as e:
            self.print_status(f"Failed to start port-forward: {e}", "error")
            return False
    
    def stop_port_forward(self):
        """Stop the port-forward process"""
        if self.port_forward_process:
            self.port_forward_process.terminate()
            try:
                self.port_forward_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.port_forward_process.kill()
            self.port_forward_process = None
            self.print_status("Port-forward stopped", "info")
    
    async def connect_to_mcp_server(self):
        """Connect to the MCP server via port-forward"""
        server_url = f"http://localhost:{self.local_port}/mcp"
        
        try:
            self.print_status(f"Connecting to MCP server at {server_url}...", "info")
            
            # Create the client transport and connect
            transport = await self.exit_stack.enter_async_context(
                streamablehttp_client(server_url)
            )
            
            # Create the session
            self.session = await self.exit_stack.enter_async_context(
                ClientSession(transport[0], transport[1])
            )
            
            # Initialize the connection
            await self.session.initialize()
            
            # List available tools
            tools_result = await self.session.list_tools()
            self.print_status(f"Connected! Available tools: {len(tools_result.tools)}", "success")
            
            for tool in tools_result.tools[:5]:  # Show first 5 tools
                self.print_status(f"   🔧 {tool.name}", "info")
            if len(tools_result.tools) > 5:
                self.print_status(f"   ... and {len(tools_result.tools) - 5} more", "info")
            
            return True
            
        except Exception as e:
            self.print_status(f"Failed to connect to MCP server: {e}", "error")
            return False
    
    async def test_mcp_functionality(self):
        """Test basic MCP functionality"""
        if not self.session:
            self.print_status("Not connected to MCP server", "error")
            return False
        
        self.print_status("Testing MCP functionality...", "info")
        
        try:
            # Test 1: Get applications
            self.print_status("📱 Getting applications...", "info")
            result = await self.session.call_tool("get_applications", arguments={})
            apps_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
            self.print_status(f"   Found applications: {apps_text[:100]}...", "success")
            
            # Test 2: Get windows
            self.print_status("🪟 Getting windows...", "info")
            result = await self.session.call_tool("get_windows", arguments={})
            windows_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
            self.print_status(f"   Found windows: {windows_text[:100]}...", "success")
            
            # Test 3: Get focused window tree (if any window is focused)
            self.print_status("🌳 Getting focused window tree...", "info")
            try:
                result = await self.session.call_tool("get_focused_window_tree", arguments={})
                tree_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                self.print_status(f"   Window tree: {tree_text[:100]}...", "success")
            except Exception as e:
                self.print_status(f"   No focused window or error: {e}", "warning")
            
            self.print_status("MCP functionality test completed!", "success")
            return True
            
        except Exception as e:
            self.print_status(f"MCP functionality test failed: {e}", "error")
            return False
    
    async def interactive_mode(self):
        """Run an interactive session"""
        self.print_status("🤖 Kubernetes MCP Client - Interactive Mode", "info")
        print("=" * 70)
        print(f"Connected to Terminator MCP Agent in Kubernetes")
        print(f"Namespace: {self.namespace}")
        print(f"Service: {self.service_name}")
        print("Available commands:")
        print("  - 'test' - Run basic functionality tests")
        print("  - 'apps' - List applications")
        print("  - 'windows' - List windows")
        print("  - 'tree' - Get focused window tree")
        print("  - 'tools' - List all available tools")
        print("  - 'exit' or 'quit' - End session")
        print("=" * 70)
        
        while True:
            try:
                # Get user input
                user_input = input("\n💬 You: ").strip()
                
                if user_input.lower() in ['exit', 'quit']:
                    self.print_status("Goodbye!", "info")
                    break
                
                if not user_input:
                    continue
                
                # Handle special commands
                if user_input.lower() == 'test':
                    await self.test_mcp_functionality()
                    continue
                
                if user_input.lower() == 'tools':
                    tools_result = await self.session.list_tools()
                    self.print_status(f"Available Tools ({len(tools_result.tools)}):", "info")
                    for i, tool in enumerate(tools_result.tools, 1):
                        print(f"  {i:2d}. {tool.name}")
                        if tool.description:
                            print(f"      {tool.description}")
                    continue
                
                if user_input.lower() == 'apps':
                    result = await self.session.call_tool("get_applications", arguments={})
                    apps_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                    print(f"\n📱 Applications:\n{apps_text}")
                    continue
                
                if user_input.lower() == 'windows':
                    result = await self.session.call_tool("get_windows", arguments={})
                    windows_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                    print(f"\n🪟 Windows:\n{windows_text}")
                    continue
                
                if user_input.lower() == 'tree':
                    try:
                        result = await self.session.call_tool("get_focused_window_tree", arguments={})
                        tree_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                        print(f"\n🌳 Focused Window Tree:\n{tree_text}")
                    except Exception as e:
                        self.print_status(f"Error getting window tree: {e}", "error")
                    continue
                
                self.print_status("Unknown command. Use 'test', 'apps', 'windows', 'tree', 'tools', or 'exit'", "warning")
                
            except KeyboardInterrupt:
                self.print_status("\nGoodbye!", "info")
                break
            except Exception as e:
                self.print_status(f"Error: {e}", "error")
    
    async def cleanup(self):
        """Clean up resources"""
        try:
            await self.exit_stack.aclose()
        except Exception:
            pass
        
        self.stop_port_forward()


async def main():
    parser = argparse.ArgumentParser(description="Kubernetes MCP Client for Terminator")
    parser.add_argument(
        "--namespace",
        default="terminator",
        help="Kubernetes namespace (default: terminator)"
    )
    parser.add_argument(
        "--service",
        default="terminator-mcp-service",
        help="Service name (default: terminator-mcp-service)"
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8080,
        help="Service port (default: 8080)"
    )
    parser.add_argument(
        "--local-port",
        type=int,
        default=8080,
        help="Local port for port-forward (default: 8080)"
    )
    parser.add_argument(
        "--test-only",
        action="store_true",
        help="Run tests and exit"
    )
    
    args = parser.parse_args()
    
    client = KubernetesMCPClient(args.namespace, args.service, args.port)
    client.local_port = args.local_port
    
    try:
        # Check if service exists
        if not client.check_service_exists():
            return 1
        
        # Start port-forward
        if not client.start_port_forward():
            return 1
        
        # Connect to MCP server
        if not await client.connect_to_mcp_server():
            return 1
        
        if args.test_only:
            # Run tests and exit
            success = await client.test_mcp_functionality()
            return 0 if success else 1
        else:
            # Run interactive session
            await client.interactive_mode()
            return 0
        
    except KeyboardInterrupt:
        client.print_status("Interrupted by user", "info")
        return 0
    except Exception as e:
        client.print_status(f"Unexpected error: {e}", "error")
        return 1
    finally:
        # Clean up
        await client.cleanup()


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    exit(exit_code)
