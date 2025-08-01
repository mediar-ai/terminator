#!/usr/bin/env python3
"""
Kubernetes Deployment Validation Script for Terminator MCP Agent
Tests the complete Kubernetes deployment functionality

Prerequisites:
pip install kubernetes requests

Usage:
python validate-k8s-deployment.py --namespace terminator
"""

import argparse
import json
import requests
import time
import sys
import subprocess
from typing import Dict, Any, Optional, List
from kubernetes import client, config
from kubernetes.client.rest import ApiException


class KubernetesValidator:
    def __init__(self, namespace: str = "terminator", app_name: str = "terminator-mcp-agent"):
        self.namespace = namespace
        self.app_name = app_name
        
        # Initialize Kubernetes client
        try:
            config.load_incluster_config()  # Try in-cluster config first
        except:
            try:
                config.load_kube_config()  # Fall back to local kubeconfig
            except Exception as e:
                self.print_status(f"Failed to load Kubernetes config: {e}", "error")
                sys.exit(1)
        
        self.v1 = client.CoreV1Api()
        self.apps_v1 = client.AppsV1Api()
        self.networking_v1 = client.NetworkingV1Api()
        self.autoscaling_v2 = client.AutoscalingV2Api()
        
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
    
    def check_namespace(self) -> bool:
        """Check if namespace exists"""
        self.print_status(f"Checking namespace '{self.namespace}'...", "info")
        
        try:
            namespace = self.v1.read_namespace(name=self.namespace)
            self.print_status(f"Namespace '{self.namespace}' exists", "success")
            return True
        except ApiException as e:
            if e.status == 404:
                self.print_status(f"Namespace '{self.namespace}' not found", "error")
            else:
                self.print_status(f"Error checking namespace: {e}", "error")
            return False
    
    def check_windows_nodes(self) -> bool:
        """Check if Windows nodes are available"""
        self.print_status("Checking Windows nodes...", "info")
        
        try:
            nodes = self.v1.list_node(label_selector="kubernetes.io/os=windows")
            windows_nodes = [node for node in nodes.items if node.status.conditions[-1].type == "Ready" and node.status.conditions[-1].status == "True"]
            
            if windows_nodes:
                self.print_status(f"Found {len(windows_nodes)} ready Windows node(s)", "success")
                for node in windows_nodes:
                    self.print_status(f"  - {node.metadata.name} ({node.status.node_info.os_image})", "info")
                return True
            else:
                self.print_status("No ready Windows nodes found", "error")
                return False
                
        except ApiException as e:
            self.print_status(f"Error checking Windows nodes: {e}", "error")
            return False
    
    def check_deployment(self) -> bool:
        """Check deployment status"""
        self.print_status("Checking deployment status...", "info")
        
        try:
            deployment = self.apps_v1.read_namespaced_deployment(
                name=self.app_name, 
                namespace=self.namespace
            )
            
            # Check deployment conditions
            available_replicas = deployment.status.available_replicas or 0
            desired_replicas = deployment.spec.replicas or 0
            
            if available_replicas >= desired_replicas and available_replicas > 0:
                self.print_status(f"Deployment is healthy ({available_replicas}/{desired_replicas} replicas available)", "success")
                return True
            else:
                self.print_status(f"Deployment is not healthy ({available_replicas}/{desired_replicas} replicas available)", "error")
                return False
                
        except ApiException as e:
            if e.status == 404:
                self.print_status(f"Deployment '{self.app_name}' not found", "error")
            else:
                self.print_status(f"Error checking deployment: {e}", "error")
            return False
    
    def check_pods(self) -> bool:
        """Check pod status"""
        self.print_status("Checking pod status...", "info")
        
        try:
            pods = self.v1.list_namespaced_pod(
                namespace=self.namespace,
                label_selector=f"app={self.app_name}"
            )
            
            if not pods.items:
                self.print_status("No pods found", "error")
                return False
            
            running_pods = 0
            for pod in pods.items:
                pod_status = pod.status.phase
                if pod_status == "Running":
                    running_pods += 1
                    self.print_status(f"  Pod {pod.metadata.name}: {pod_status} on {pod.spec.node_name}", "success")
                else:
                    self.print_status(f"  Pod {pod.metadata.name}: {pod_status}", "warning")
            
            if running_pods > 0:
                self.print_status(f"{running_pods}/{len(pods.items)} pods are running", "success")
                return True
            else:
                self.print_status("No pods are running", "error")
                return False
                
        except ApiException as e:
            self.print_status(f"Error checking pods: {e}", "error")
            return False
    
    def check_services(self) -> bool:
        """Check service configuration"""
        self.print_status("Checking services...", "info")
        
        try:
            services = self.v1.list_namespaced_service(namespace=self.namespace)
            mcp_services = [svc for svc in services.items if self.app_name in svc.metadata.name]
            
            if not mcp_services:
                self.print_status("No MCP services found", "error")
                return False
            
            for service in mcp_services:
                service_type = service.spec.type
                ports = [f"{port.port}:{port.target_port}" for port in service.spec.ports]
                self.print_status(f"  Service {service.metadata.name}: {service_type} [{', '.join(ports)}]", "success")
                
                # Check endpoints
                try:
                    endpoints = self.v1.read_namespaced_endpoints(
                        name=service.metadata.name,
                        namespace=self.namespace
                    )
                    
                    if endpoints.subsets and endpoints.subsets[0].addresses:
                        endpoint_count = len(endpoints.subsets[0].addresses)
                        self.print_status(f"    {endpoint_count} endpoint(s) available", "success")
                    else:
                        self.print_status(f"    No endpoints available", "warning")
                        
                except ApiException:
                    self.print_status(f"    Could not check endpoints", "warning")
            
            return True
            
        except ApiException as e:
            self.print_status(f"Error checking services: {e}", "error")
            return False
    
    def check_hpa(self) -> bool:
        """Check HorizontalPodAutoscaler"""
        self.print_status("Checking HPA...", "info")
        
        try:
            hpa = self.autoscaling_v2.read_namespaced_horizontal_pod_autoscaler(
                name=f"{self.app_name}-hpa",
                namespace=self.namespace
            )
            
            current_replicas = hpa.status.current_replicas or 0
            desired_replicas = hpa.status.desired_replicas or 0
            min_replicas = hpa.spec.min_replicas or 0
            max_replicas = hpa.spec.max_replicas or 0
            
            self.print_status(f"HPA: {current_replicas} current, {desired_replicas} desired (min: {min_replicas}, max: {max_replicas})", "success")
            return True
            
        except ApiException as e:
            if e.status == 404:
                self.print_status("HPA not found (optional)", "warning")
            else:
                self.print_status(f"Error checking HPA: {e}", "warning")
            return True  # HPA is optional
    
    def test_pod_health(self) -> bool:
        """Test health endpoints on pods"""
        self.print_status("Testing pod health endpoints...", "info")
        
        try:
            pods = self.v1.list_namespaced_pod(
                namespace=self.namespace,
                label_selector=f"app={self.app_name}",
                field_selector="status.phase=Running"
            )
            
            if not pods.items:
                self.print_status("No running pods to test", "error")
                return False
            
            success_count = 0
            for pod in pods.items:
                pod_name = pod.metadata.name
                
                # Test health endpoint using kubectl exec
                try:
                    cmd = [
                        "kubectl", "exec", "-n", self.namespace, pod_name, "--",
                        "powershell", "-c",
                        "try { $r = iwr http://localhost:8080/health -UseBasicParsing; if ($r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }"
                    ]
                    
                    result = subprocess.run(cmd, capture_output=True, timeout=10)
                    
                    if result.returncode == 0:
                        self.print_status(f"  Pod {pod_name}: Health endpoint OK", "success")
                        success_count += 1
                    else:
                        self.print_status(f"  Pod {pod_name}: Health endpoint failed", "error")
                        
                except subprocess.TimeoutExpired:
                    self.print_status(f"  Pod {pod_name}: Health check timed out", "error")
                except Exception as e:
                    self.print_status(f"  Pod {pod_name}: Health check error: {e}", "error")
            
            if success_count > 0:
                self.print_status(f"{success_count}/{len(pods.items)} pods passed health check", "success")
                return True
            else:
                self.print_status("No pods passed health check", "error")
                return False
                
        except ApiException as e:
            self.print_status(f"Error testing pod health: {e}", "error")
            return False
    
    def test_service_connectivity(self) -> bool:
        """Test service connectivity"""
        self.print_status("Testing service connectivity...", "info")
        
        try:
            # Get a running pod to test from
            pods = self.v1.list_namespaced_pod(
                namespace=self.namespace,
                label_selector=f"app={self.app_name}",
                field_selector="status.phase=Running"
            )
            
            if not pods.items:
                self.print_status("No running pods to test from", "error")
                return False
            
            test_pod = pods.items[0].metadata.name
            service_name = f"{self.app_name.replace('terminator-mcp-agent', 'terminator-mcp-service')}"
            
            # Test service connectivity
            cmd = [
                "kubectl", "exec", "-n", self.namespace, test_pod, "--",
                "powershell", "-c",
                f"try {{ $r = iwr http://{service_name}:8080/health -UseBasicParsing; if ($r.StatusCode -eq 200) {{ exit 0 }} else {{ exit 1 }} }} catch {{ exit 1 }}"
            ]
            
            result = subprocess.run(cmd, capture_output=True, timeout=15)
            
            if result.returncode == 0:
                self.print_status(f"Service connectivity test passed", "success")
                return True
            else:
                self.print_status(f"Service connectivity test failed", "error")
                return False
                
        except Exception as e:
            self.print_status(f"Error testing service connectivity: {e}", "error")
            return False
    
    def test_external_access(self) -> bool:
        """Test external access via port-forward"""
        self.print_status("Testing external access (port-forward)...", "info")
        
        try:
            # Start port-forward in background
            port_forward_cmd = [
                "kubectl", "port-forward", "-n", self.namespace,
                "svc/terminator-mcp-service", "8080:8080"
            ]
            
            import threading
            import time
            
            def run_port_forward():
                subprocess.run(port_forward_cmd, capture_output=True)
            
            # Start port-forward in background thread
            port_forward_thread = threading.Thread(target=run_port_forward)
            port_forward_thread.daemon = True
            port_forward_thread.start()
            
            # Wait for port-forward to establish
            time.sleep(3)
            
            # Test the connection
            try:
                response = requests.get("http://localhost:8080/health", timeout=5)
                if response.status_code == 200:
                    self.print_status("External access test passed", "success")
                    return True
                else:
                    self.print_status(f"External access test failed (status: {response.status_code})", "error")
                    return False
            except requests.exceptions.RequestException as e:
                self.print_status(f"External access test failed: {e}", "error")
                return False
                
        except Exception as e:
            self.print_status(f"Error testing external access: {e}", "error")
            return False
    
    def run_validation(self) -> Dict[str, bool]:
        """Run all validation tests"""
        print("🧪 Kubernetes Deployment Validation")
        print("=" * 60)
        print(f"Namespace: {self.namespace}")
        print(f"App: {self.app_name}")
        print()
        
        tests = {
            "Namespace Check": self.check_namespace,
            "Windows Nodes": self.check_windows_nodes,
            "Deployment Status": self.check_deployment,
            "Pod Status": self.check_pods,
            "Service Configuration": self.check_services,
            "HPA Configuration": self.check_hpa,
            "Pod Health Endpoints": self.test_pod_health,
            "Service Connectivity": self.test_service_connectivity,
            "External Access": self.test_external_access
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
        print("=" * 40)
        
        passed_tests = sum(results.values())
        total_tests = len(results)
        
        for test_name, passed in results.items():
            status = "success" if passed else "error"
            self.print_status(f"{test_name}: {'PASS' if passed else 'FAIL'}", status)
        
        print()
        
        if passed_tests == total_tests:
            self.print_status(f"All {total_tests} tests passed! 🎉", "success")
            self.print_status("Kubernetes deployment is working correctly", "success")
            print()
            print("Next steps:")
            print("• Test with MCP client: kubectl port-forward -n terminator svc/terminator-mcp-service 8080:8080")
            print("• Then run: python examples/docker_mcp_simple.py --server-url http://localhost:8080/mcp")
            return True
        else:
            self.print_status(f"{passed_tests}/{total_tests} tests passed", "warning")
            print()
            print("Troubleshooting:")
            print("• Check pod logs: kubectl logs -n terminator -l app=terminator-mcp-agent")
            print("• Check events: kubectl get events -n terminator --sort-by='.lastTimestamp'")
            print("• Check node status: kubectl describe nodes -l kubernetes.io/os=windows")
            return False


def main():
    parser = argparse.ArgumentParser(description="Validate Kubernetes deployment")
    parser.add_argument(
        "--namespace",
        default="terminator",
        help="Kubernetes namespace (default: terminator)"
    )
    parser.add_argument(
        "--app-name",
        default="terminator-mcp-agent",
        help="Application name (default: terminator-mcp-agent)"
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose output"
    )
    
    args = parser.parse_args()
    
    validator = KubernetesValidator(args.namespace, args.app_name)
    results = validator.run_validation()
    success = validator.print_summary(results)
    
    # Exit with appropriate code
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
