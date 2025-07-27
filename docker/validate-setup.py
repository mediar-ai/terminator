#!/usr/bin/env python3
"""
Validation script for Docker Windows setup
Tests the complete Terminator MCP Agent Docker container functionality

Prerequisites:
pip install requests

Usage:
python validate-setup.py --container-url http://localhost:8080
"""

import argparse
import json
import requests
import time
import sys
from typing import Dict, Any, Optional


class DockerSetupValidator:
    def __init__(self, container_url: str = "http://localhost:8080"):
        self.container_url = container_url.rstrip('/')
        self.health_url = f"{self.container_url}/health"
        self.mcp_url = f"{self.container_url}/mcp"
        self.session = requests.Session()
        self.session.timeout = 10
        
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
    
    def test_health_endpoint(self) -> bool:
        """Test the health endpoint"""
        self.print_status("Testing health endpoint...", "info")
        
        try:
            response = self.session.get(self.health_url)
            
            if response.status_code == 200:
                data = response.json()
                if data.get("status") == "ok":
                    self.print_status("Health endpoint responding correctly", "success")
                    return True
                else:
                    self.print_status(f"Health endpoint returned unexpected data: {data}", "error")
                    return False
            else:
                self.print_status(f"Health endpoint returned status {response.status_code}", "error")
                return False
                
        except requests.exceptions.ConnectionError:
            self.print_status("Cannot connect to container - is it running?", "error")
            return False
        except requests.exceptions.Timeout:
            self.print_status("Health endpoint timed out", "error")
            return False
        except Exception as e:
            self.print_status(f"Health endpoint test failed: {e}", "error")
            return False
    
    def test_mcp_endpoint(self) -> bool:
        """Test that MCP endpoint is accessible"""
        self.print_status("Testing MCP endpoint accessibility...", "info")
        
        try:
            # MCP endpoint should respond to GET requests (even if with an error)
            response = self.session.get(self.mcp_url)
            
            # Any response (even 4xx/5xx) indicates the endpoint is accessible
            if response.status_code in [200, 400, 404, 405, 500]:
                self.print_status("MCP endpoint is accessible", "success")
                return True
            else:
                self.print_status(f"MCP endpoint returned unexpected status {response.status_code}", "warning")
                return True  # Still consider this a pass as the endpoint responded
                
        except requests.exceptions.ConnectionError:
            self.print_status("Cannot connect to MCP endpoint", "error")
            return False
        except requests.exceptions.Timeout:
            self.print_status("MCP endpoint timed out", "error")
            return False
        except Exception as e:
            self.print_status(f"MCP endpoint test failed: {e}", "error")
            return False
    
    def test_container_responsiveness(self) -> bool:
        """Test overall container responsiveness"""
        self.print_status("Testing container responsiveness...", "info")
        
        # Test multiple rapid requests to ensure stability
        success_count = 0
        total_requests = 5
        
        for i in range(total_requests):
            try:
                start_time = time.time()
                response = self.session.get(self.health_url)
                response_time = time.time() - start_time
                
                if response.status_code == 200 and response_time < 5.0:
                    success_count += 1
                
                time.sleep(0.5)  # Small delay between requests
                
            except Exception:
                pass
        
        success_rate = success_count / total_requests
        
        if success_rate >= 0.8:  # 80% success rate
            self.print_status(f"Container is responsive ({success_count}/{total_requests} requests succeeded)", "success")
            return True
        else:
            self.print_status(f"Container responsiveness is poor ({success_count}/{total_requests} requests succeeded)", "warning")
            return False
    
    def test_cors_headers(self) -> bool:
        """Test that CORS headers are present"""
        self.print_status("Testing CORS configuration...", "info")
        
        try:
            # Make an OPTIONS request to check CORS headers
            response = self.session.options(self.mcp_url)
            
            cors_headers = [
                'Access-Control-Allow-Origin',
                'Access-Control-Allow-Methods',
                'Access-Control-Allow-Headers'
            ]
            
            found_cors_headers = []
            for header in cors_headers:
                if header in response.headers:
                    found_cors_headers.append(header)
            
            if found_cors_headers:
                self.print_status(f"CORS headers found: {', '.join(found_cors_headers)}", "success")
                return True
            else:
                self.print_status("No CORS headers found - may limit web client access", "warning")
                return False
                
        except Exception as e:
            self.print_status(f"CORS test failed: {e}", "warning")
            return False
    
    def run_validation(self) -> Dict[str, bool]:
        """Run all validation tests"""
        print("🧪 Docker Windows Setup Validation")
        print("=" * 50)
        print(f"Testing container at: {self.container_url}")
        print()
        
        tests = {
            "Health Endpoint": self.test_health_endpoint,
            "MCP Endpoint": self.test_mcp_endpoint,
            "Container Responsiveness": self.test_container_responsiveness,
            "CORS Configuration": self.test_cors_headers
        }
        
        results = {}
        
        for test_name, test_func in tests.items():
            try:
                results[test_name] = test_func()
            except Exception as e:
                self.print_status(f"{test_name} test crashed: {e}", "error")
                results[test_name] = False
            
            print()  # Add spacing between tests
        
        return results
    
    def print_summary(self, results: Dict[str, bool]):
        """Print validation summary"""
        print("📊 Validation Summary")
        print("=" * 30)
        
        passed_tests = sum(results.values())
        total_tests = len(results)
        
        for test_name, passed in results.items():
            status = "success" if passed else "error"
            self.print_status(f"{test_name}: {'PASS' if passed else 'FAIL'}", status)
        
        print()
        
        if passed_tests == total_tests:
            self.print_status(f"All {total_tests} tests passed! 🎉", "success")
            self.print_status("Docker Windows setup is working correctly", "success")
            print()
            print("Next steps:")
            print("• Test with Python MCP client: python examples/docker_mcp_simple.py --demo")
            print("• Connect your MCP client to:", self.mcp_url)
            return True
        else:
            self.print_status(f"{passed_tests}/{total_tests} tests passed", "warning")
            print()
            print("Troubleshooting:")
            print("• Check container logs: docker logs terminator-mcp-windows")
            print("• Verify container is running: docker ps")
            print("• Run setup test: .\\docker\\test-docker-setup.ps1")
            return False


def main():
    parser = argparse.ArgumentParser(description="Validate Docker Windows setup")
    parser.add_argument(
        "--container-url",
        default="http://localhost:8080",
        help="Container URL (default: http://localhost:8080)"
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose output"
    )
    
    args = parser.parse_args()
    
    validator = DockerSetupValidator(args.container_url)
    results = validator.run_validation()
    success = validator.print_summary(results)
    
    # Exit with appropriate code
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
