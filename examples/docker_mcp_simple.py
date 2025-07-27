#!/usr/bin/env python3
"""
Simple Docker MCP Client for Terminator Desktop Automation
Connects to Terminator MCP Agent running in a Docker Windows container via HTTP
No AI dependencies required - just basic MCP functionality

Prerequisites:
pip install mcp

Usage:
python docker_mcp_simple.py --server-url http://localhost:8080/mcp
"""

import asyncio
import argparse
import json
from typing import Optional
from contextlib import AsyncExitStack

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client


class SimpleMCPClient:
    def __init__(self, server_url: str = "http://localhost:8080/mcp"):
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        self.server_url = server_url
    
    async def connect(self):
        """Connect to the MCP server"""
        try:
            print(f"🔌 Connecting to {self.server_url}...")
            
            # Create the client transport and connect
            transport = await self.exit_stack.enter_async_context(
                streamablehttp_client(self.server_url)
            )
            
            # Create the session
            self.session = await self.exit_stack.enter_async_context(
                ClientSession(transport[0], transport[1])
            )
            
            # Initialize the connection
            await self.session.initialize()
            
            print("✅ Connected successfully!")
            return True
            
        except Exception as e:
            print(f"❌ Connection failed: {e}")
            print("💡 Make sure the Docker container is running:")
            print("   docker-compose -f docker/docker-compose.windows.yml up")
            return False
    
    async def list_tools(self):
        """List all available tools"""
        if not self.session:
            print("❌ Not connected")
            return
        
        try:
            tools_result = await self.session.list_tools()
            print(f"\n🔧 Available Tools ({len(tools_result.tools)}):")
            print("-" * 50)
            
            for i, tool in enumerate(tools_result.tools, 1):
                print(f"{i:2d}. {tool.name}")
                if tool.description:
                    print(f"     {tool.description}")
                print()
            
        except Exception as e:
            print(f"❌ Error listing tools: {e}")
    
    async def call_tool(self, tool_name: str, arguments: dict = None):
        """Call a specific tool"""
        if not self.session:
            print("❌ Not connected")
            return None
        
        if arguments is None:
            arguments = {}
        
        try:
            print(f"🔧 Calling tool: {tool_name}")
            if arguments:
                print(f"   Arguments: {json.dumps(arguments, indent=2)}")
            
            result = await self.session.call_tool(tool_name, arguments=arguments)
            
            # Extract and display results
            result_text = []
            for item in result.content:
                if hasattr(item, 'text'):
                    result_text.append(item.text)
                elif hasattr(item, 'data'):
                    result_text.append(str(item.data))
            
            output = "\n".join(result_text) if result_text else "Tool executed successfully"
            print(f"✅ Result:\n{output}")
            return output
            
        except Exception as e:
            print(f"❌ Error calling tool: {e}")
            return None
    
    async def run_demo(self):
        """Run a demonstration of basic functionality"""
        print("\n🎯 Running Basic Demo...")
        print("=" * 50)
        
        # Demo 1: Get applications
        print("\n1️⃣ Getting available applications...")
        await self.call_tool("get_applications")
        
        # Demo 2: Get windows
        print("\n2️⃣ Getting open windows...")
        await self.call_tool("get_windows")
        
        # Demo 3: Try to get focused window tree
        print("\n3️⃣ Getting focused window tree...")
        await self.call_tool("get_focused_window_tree")
        
        # Demo 4: Try to open Calculator (if available)
        print("\n4️⃣ Attempting to open Calculator...")
        await self.call_tool("open_application", {"path": "calc.exe"})
        
        # Wait a moment for the app to open
        await asyncio.sleep(2)
        
        # Demo 5: Get windows again to see if Calculator opened
        print("\n5️⃣ Checking windows after opening Calculator...")
        await self.call_tool("get_windows")
        
        print("\n✅ Demo completed!")
    
    async def interactive_mode(self):
        """Run interactive mode"""
        print("\n🤖 Simple MCP Client - Interactive Mode")
        print("=" * 50)
        print("Available commands:")
        print("  tools          - List all available tools")
        print("  demo           - Run basic demonstration")
        print("  apps           - Get applications")
        print("  windows        - Get windows")
        print("  tree           - Get focused window tree")
        print("  open <app>     - Open application (e.g., 'open calc.exe')")
        print("  call <tool>    - Call a tool with no arguments")
        print("  help           - Show this help")
        print("  exit/quit      - Exit the program")
        print("=" * 50)
        
        while True:
            try:
                user_input = input("\n💬 Command: ").strip()
                
                if not user_input:
                    continue
                
                if user_input.lower() in ['exit', 'quit']:
                    print("👋 Goodbye!")
                    break
                
                if user_input.lower() == 'help':
                    print("\n📖 Available commands:")
                    print("  tools, demo, apps, windows, tree, open <app>, call <tool>, help, exit")
                    continue
                
                if user_input.lower() == 'tools':
                    await self.list_tools()
                    continue
                
                if user_input.lower() == 'demo':
                    await self.run_demo()
                    continue
                
                if user_input.lower() == 'apps':
                    await self.call_tool("get_applications")
                    continue
                
                if user_input.lower() == 'windows':
                    await self.call_tool("get_windows")
                    continue
                
                if user_input.lower() == 'tree':
                    await self.call_tool("get_focused_window_tree")
                    continue
                
                if user_input.lower().startswith('open '):
                    app_name = user_input[5:].strip()
                    if app_name:
                        await self.call_tool("open_application", {"path": app_name})
                    else:
                        print("❌ Please specify an application name")
                    continue
                
                if user_input.lower().startswith('call '):
                    tool_name = user_input[5:].strip()
                    if tool_name:
                        await self.call_tool(tool_name)
                    else:
                        print("❌ Please specify a tool name")
                    continue
                
                print(f"❌ Unknown command: {user_input}")
                print("💡 Type 'help' for available commands")
                
            except KeyboardInterrupt:
                print("\n👋 Goodbye!")
                break
            except Exception as e:
                print(f"❌ Error: {e}")
    
    async def cleanup(self):
        """Clean up resources"""
        try:
            await self.exit_stack.aclose()
        except Exception:
            pass


async def main():
    parser = argparse.ArgumentParser(description="Simple Docker MCP Client")
    parser.add_argument(
        "--server-url",
        default="http://localhost:8080/mcp",
        help="MCP server URL (default: http://localhost:8080/mcp)"
    )
    parser.add_argument(
        "--demo",
        action="store_true",
        help="Run demo and exit"
    )
    parser.add_argument(
        "--list-tools",
        action="store_true",
        help="List tools and exit"
    )
    
    args = parser.parse_args()
    
    client = SimpleMCPClient(args.server_url)
    
    try:
        # Connect to server
        if not await client.connect():
            return 1
        
        if args.list_tools:
            await client.list_tools()
        elif args.demo:
            await client.run_demo()
        else:
            await client.interactive_mode()
        
        return 0
        
    except Exception as e:
        print(f"❌ Unexpected error: {e}")
        return 1
    finally:
        await client.cleanup()


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    exit(exit_code)
