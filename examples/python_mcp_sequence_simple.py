#!/usr/bin/env python3
"""
Simple MCP Execute Sequence Demo - Shows step-by-step execution tracking
A minimal example showing how execute_sequence provides detailed progress

pip install mcp
"""

import asyncio
import json
import os
from typing import Optional
from contextlib import AsyncExitStack

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client


class SimpleSequenceClient:
    def __init__(self):
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
    
    async def connect(self, server_command: str = "target/release/terminator-mcp-agent"):
        """Connect to the MCP server"""
        print(f"🔌 Connecting to {server_command}...")
        
        server_params = StdioServerParameters(
            command=server_command,
            args=[],
            env=None
        )
        
        transport = await self.exit_stack.enter_async_context(
            stdio_client(server_params)
        )
        
        self.session = await self.exit_stack.enter_async_context(
            ClientSession(transport[0], transport[1])
        )
        
        await self.session.initialize()
        print("✅ Connected!\n")
    
    async def run_simple_sequence(self):
        """Run a simple sequence that demonstrates step tracking"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        # Simple sequence: Get applications, take screenshot, get clipboard
        sequence = [
            {
                "tool_name": "get_applications",
                "arguments": {
                    "include_tree": False
                }
            },
            {
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 1000
                }
            },
            {
                "tool_name": "capture_screen",
                "arguments": {}
            },
            {
                "tool_name": "get_clipboard",
                "arguments": {}
            }
        ]
        
        print("🚀 Executing sequence with 4 steps:")
        print("   1. Get running applications")
        print("   2. Wait 1 second")
        print("   3. Capture screenshot")
        print("   4. Get clipboard content\n")
        
        # Convert to JSON string
        tools_json = json.dumps(sequence)
        
        # Call execute_sequence
        print("📡 Calling execute_sequence tool...\n")
        
        result = await self.session.call_tool(
            "execute_sequence",
            arguments={
                "tools_json": tools_json,
                "stop_on_error": True,
                "include_detailed_results": True
            }
        )
        
        # Parse and display results
        if result.content and len(result.content) > 0:
            content = result.content[0]
            if hasattr(content, 'text'):
                result_data = json.loads(content.text)
            else:
                result_data = content
            
            # Display execution plan
            if 'execution_plan' in result_data:
                print("📋 EXECUTION PLAN:")
                plan = result_data['execution_plan']
                for step in plan.get('steps', []):
                    print(f"   Step {step['step']}: {step['tool_name']}")
                print()
            
            # Display step results
            if 'step_results' in result_data:
                print("🔄 STEP-BY-STEP RESULTS:\n")
                
                for step in result_data['step_results']:
                    print(f"{'='*50}")
                    print(f"Step {step['step']}: {step['tool_name']}")
                    print(f"Status: {step['status']}")
                    print(f"Duration: {step.get('duration_ms', 'N/A')}ms")
                    print(f"Progress: {step.get('progress', 'N/A')}")
                    
                    if 'error' in step:
                        print(f"❌ Error: {step['error']}")
                    elif 'result' in step and isinstance(step['result'], dict):
                        result_content = step['result'].get('content', [])
                        if result_content:
                            print(f"✅ Result preview:")
                            # Show a preview of the result
                            preview = json.dumps(result_content[0] if result_content else {}, indent=2)
                            preview_lines = preview.split('\n')[:5]  # First 5 lines
                            for line in preview_lines:
                                print(f"   {line}")
                            if len(preview.split('\n')) > 5:
                                print("   ...")
                    print()
            
            # Display summary
            if 'execution_summary' in result_data:
                summary = result_data['execution_summary']
                print("📊 EXECUTION SUMMARY:")
                print(f"   Total steps: {summary.get('total_steps', 0)}")
                print(f"   Successful: {summary.get('successful_steps', 0)}")
                print(f"   Failed: {summary.get('failed_steps', 0)}")
                print(f"   Total duration: {summary.get('total_duration_ms', 0)}ms")
                print(f"   Status: {result_data.get('status', 'unknown')}")
    
    async def cleanup(self):
        """Clean up resources"""
        await self.exit_stack.aclose()


async def main():
    """Main entry point"""
    client = SimpleSequenceClient()
    
    try:
        await client.connect()
        await client.run_simple_sequence()
    finally:
        await client.cleanup()


if __name__ == "__main__":
    print("="*60)
    print("🤖 Simple MCP Execute Sequence Demo")
    print("="*60)
    print("\nThis demo shows how execute_sequence provides detailed")
    print("step-by-step execution information for each tool in the sequence.\n")
    
    if not os.path.exists("target/release/terminator-mcp-agent"):
        print("⚠️  Warning: terminator-mcp-agent not found")
        print("Build it with: cargo build --release --bin terminator-mcp-agent\n")
    
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n👋 Demo interrupted")