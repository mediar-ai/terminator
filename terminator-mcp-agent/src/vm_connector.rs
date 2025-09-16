// VM Connection abstraction layer for supporting different VM environments
// Supports local VMs (Hyper-V, VMware) and cloud VMs (Azure)

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VMConfig {
    Local {
        hypervisor: HypervisorType,
        vm_name: String,
        connection_method: LocalConnectionMethod,
    },
    Azure {
        subscription_id: String,
        resource_group: String,
        vm_name: String,
        connection_method: AzureConnectionMethod,
    },
    Remote {
        host: String,
        port: u16,
        api_key: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HypervisorType {
    HyperV,
    VMware,
    VirtualBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalConnectionMethod {
    VMConnect,      // Hyper-V VM Connect
    VMwareRemote,   // VMware Remote Console
    RDP {
        port: u16,
        credentials: Option<Credentials>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AzureConnectionMethod {
    Bastion {
        bastion_host: String,
    },
    DirectRDP {
        public_ip: String,
        port: u16,
        credentials: Option<Credentials>,
    },
    SerialConsole,
    RemoteAgent {
        port: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    pub use_key_vault: bool,
    pub key_vault_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMConnectionInfo {
    pub vm_name: String,
    pub status: VMStatus,
    pub connection_string: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VMStatus {
    Running,
    Stopped,
    Starting,
    Stopping,
    Unknown,
}

#[async_trait]
pub trait VMConnector: Send + Sync {
    async fn connect(&self) -> Result<VMConnectionInfo>;
    async fn disconnect(&self) -> Result<()>;
    async fn get_status(&self) -> Result<VMStatus>;
    async fn start_vm(&self) -> Result<()>;
    async fn stop_vm(&self) -> Result<()>;
    async fn restart_vm(&self) -> Result<()>;
    async fn deploy_agent(&self, agent_path: &str) -> Result<()>;
}

/// Local VM connector for Hyper-V, VMware, VirtualBox
pub struct LocalVMConnector {
    config: VMConfig,
}

impl LocalVMConnector {
    pub fn new(hypervisor: HypervisorType, vm_name: String) -> Self {
        Self {
            config: VMConfig::Local {
                hypervisor,
                vm_name,
                connection_method: LocalConnectionMethod::RDP {
                    port: 3389,
                    credentials: None,
                },
            },
        }
    }

    async fn get_hyperv_vm_state(&self, vm_name: &str) -> Result<VMStatus> {
        let output = tokio::process::Command::new("powershell")
            .args(&[
                "-Command",
                &format!("(Get-VM -Name '{}').State", vm_name),
            ])
            .output()
            .await
            .context("Failed to get Hyper-V VM state")?;

        let state = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(match state.as_str() {
            "Running" => VMStatus::Running,
            "Off" => VMStatus::Stopped,
            "Starting" => VMStatus::Starting,
            "Stopping" => VMStatus::Stopping,
            _ => VMStatus::Unknown,
        })
    }
}

#[async_trait]
impl VMConnector for LocalVMConnector {
    async fn connect(&self) -> Result<VMConnectionInfo> {
        match &self.config {
            VMConfig::Local { hypervisor, vm_name, connection_method } => {
                let status = self.get_status().await?;

                let connection_string = match connection_method {
                    LocalConnectionMethod::VMConnect => {
                        // Launch Hyper-V VM Connect
                        format!("vmconnect.exe localhost '{}'", vm_name)
                    }
                    LocalConnectionMethod::VMwareRemote => {
                        // VMware remote console
                        format!("vmrc://localhost/{}", vm_name)
                    }
                    LocalConnectionMethod::RDP { port, .. } => {
                        // Get VM IP address
                        let ip = self.get_vm_ip(vm_name).await?;
                        format!("mstsc.exe /v:{}:{}", ip, port)
                    }
                };

                Ok(VMConnectionInfo {
                    vm_name: vm_name.clone(),
                    status,
                    connection_string,
                    metadata: HashMap::new(),
                })
            }
            _ => Err(anyhow::anyhow!("Invalid configuration for local VM connector")),
        }
    }

    async fn disconnect(&self) -> Result<()> {
        // Close any active connections
        Ok(())
    }

    async fn get_status(&self) -> Result<VMStatus> {
        match &self.config {
            VMConfig::Local { hypervisor, vm_name, .. } => {
                match hypervisor {
                    HypervisorType::HyperV => self.get_hyperv_vm_state(vm_name).await,
                    HypervisorType::VMware => {
                        // VMware status check
                        Ok(VMStatus::Unknown)
                    }
                    HypervisorType::VirtualBox => {
                        // VirtualBox status check
                        Ok(VMStatus::Unknown)
                    }
                }
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn start_vm(&self) -> Result<()> {
        match &self.config {
            VMConfig::Local { hypervisor, vm_name, .. } => {
                match hypervisor {
                    HypervisorType::HyperV => {
                        tokio::process::Command::new("powershell")
                            .args(&[
                                "-Command",
                                &format!("Start-VM -Name '{}'", vm_name),
                            ])
                            .status()
                            .await?;
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn stop_vm(&self) -> Result<()> {
        match &self.config {
            VMConfig::Local { hypervisor, vm_name, .. } => {
                match hypervisor {
                    HypervisorType::HyperV => {
                        tokio::process::Command::new("powershell")
                            .args(&[
                                "-Command",
                                &format!("Stop-VM -Name '{}' -Force", vm_name),
                            ])
                            .status()
                            .await?;
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn restart_vm(&self) -> Result<()> {
        self.stop_vm().await?;
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        self.start_vm().await
    }

    async fn deploy_agent(&self, agent_path: &str) -> Result<()> {
        // Deploy the remote UI automation agent to the VM
        match &self.config {
            VMConfig::Local { vm_name, .. } => {
                let ip = self.get_vm_ip(vm_name).await?;

                // Copy agent files to VM
                tokio::process::Command::new("powershell")
                    .args(&[
                        "-Command",
                        &format!(
                            "Copy-Item -Path '{}' -Destination '\\\\{}\\c$\\temp\\' -Force",
                            agent_path, ip
                        ),
                    ])
                    .status()
                    .await?;

                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }
}

impl LocalVMConnector {
    async fn get_vm_ip(&self, vm_name: &str) -> Result<String> {
        // Get VM IP address from hypervisor
        let output = tokio::process::Command::new("powershell")
            .args(&[
                "-Command",
                &format!(
                    "(Get-VM -Name '{}' | Get-VMNetworkAdapter).IPAddresses | Select-Object -First 1",
                    vm_name
                ),
            ])
            .output()
            .await?;

        let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if ip.is_empty() {
            Err(anyhow::anyhow!("Could not get VM IP address"))
        } else {
            Ok(ip)
        }
    }
}

/// Azure VM connector
pub struct AzureVMConnector {
    config: VMConfig,
}

impl AzureVMConnector {
    pub fn new(subscription_id: String, resource_group: String, vm_name: String) -> Self {
        Self {
            config: VMConfig::Azure {
                subscription_id,
                resource_group,
                vm_name,
                connection_method: AzureConnectionMethod::RemoteAgent { port: 8080 },
            },
        }
    }

    async fn az_command(&self, args: &[&str]) -> Result<String> {
        let output = tokio::process::Command::new("az")
            .args(args)
            .output()
            .await
            .context("Failed to execute az command")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Azure CLI error: {}", error));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl VMConnector for AzureVMConnector {
    async fn connect(&self) -> Result<VMConnectionInfo> {
        match &self.config {
            VMConfig::Azure {
                subscription_id,
                resource_group,
                vm_name,
                connection_method,
            } => {
                let status = self.get_status().await?;

                let connection_string = match connection_method {
                    AzureConnectionMethod::Bastion { bastion_host } => {
                        format!("https://{}.bastion.azure.com", bastion_host)
                    }
                    AzureConnectionMethod::DirectRDP { public_ip, port, .. } => {
                        format!("mstsc.exe /v:{}:{}", public_ip, port)
                    }
                    AzureConnectionMethod::SerialConsole => {
                        format!(
                            "az serial-console connect --resource-group {} --name {}",
                            resource_group, vm_name
                        )
                    }
                    AzureConnectionMethod::RemoteAgent { port } => {
                        // Get public IP
                        let ip_output = self
                            .az_command(&[
                                "vm",
                                "show",
                                "-d",
                                "--resource-group",
                                resource_group,
                                "--name",
                                vm_name,
                                "--query",
                                "publicIps",
                                "-o",
                                "tsv",
                            ])
                            .await?;

                        let public_ip = ip_output.trim();
                        format!("http://{}:{}", public_ip, port)
                    }
                };

                let mut metadata = HashMap::new();
                metadata.insert("subscription_id".to_string(), subscription_id.clone());
                metadata.insert("resource_group".to_string(), resource_group.clone());

                Ok(VMConnectionInfo {
                    vm_name: vm_name.clone(),
                    status,
                    connection_string,
                    metadata,
                })
            }
            _ => Err(anyhow::anyhow!("Invalid configuration for Azure VM connector")),
        }
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn get_status(&self) -> Result<VMStatus> {
        match &self.config {
            VMConfig::Azure {
                resource_group,
                vm_name,
                ..
            } => {
                let output = self
                    .az_command(&[
                        "vm",
                        "get-instance-view",
                        "--resource-group",
                        resource_group,
                        "--name",
                        vm_name,
                        "--query",
                        "instanceView.statuses[1].displayStatus",
                        "-o",
                        "tsv",
                    ])
                    .await?;

                let status = output.trim();
                Ok(match status {
                    "VM running" => VMStatus::Running,
                    "VM stopped" | "VM deallocated" => VMStatus::Stopped,
                    "VM starting" => VMStatus::Starting,
                    "VM stopping" => VMStatus::Stopping,
                    _ => VMStatus::Unknown,
                })
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn start_vm(&self) -> Result<()> {
        match &self.config {
            VMConfig::Azure {
                resource_group,
                vm_name,
                ..
            } => {
                self.az_command(&[
                    "vm",
                    "start",
                    "--resource-group",
                    resource_group,
                    "--name",
                    vm_name,
                ])
                .await?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn stop_vm(&self) -> Result<()> {
        match &self.config {
            VMConfig::Azure {
                resource_group,
                vm_name,
                ..
            } => {
                self.az_command(&[
                    "vm",
                    "stop",
                    "--resource-group",
                    resource_group,
                    "--name",
                    vm_name,
                ])
                .await?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn restart_vm(&self) -> Result<()> {
        match &self.config {
            VMConfig::Azure {
                resource_group,
                vm_name,
                ..
            } => {
                self.az_command(&[
                    "vm",
                    "restart",
                    "--resource-group",
                    resource_group,
                    "--name",
                    vm_name,
                ])
                .await?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }

    async fn deploy_agent(&self, agent_path: &str) -> Result<()> {
        match &self.config {
            VMConfig::Azure {
                resource_group,
                vm_name,
                ..
            } => {
                // Use Azure VM extension to deploy the agent
                self.az_command(&[
                    "vm",
                    "extension",
                    "set",
                    "--resource-group",
                    resource_group,
                    "--vm-name",
                    vm_name,
                    "--name",
                    "RemoteUIAutomationAgent",
                    "--publisher",
                    "Microsoft.Compute",
                    "--version",
                    "1.0",
                    "--settings",
                    &format!(r#"{{"agentPath": "{}"}}"#, agent_path),
                ])
                .await?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Invalid configuration")),
        }
    }
}

/// Factory for creating VM connectors
pub struct VMConnectorFactory;

impl VMConnectorFactory {
    pub fn create_from_config(config: VMConfig) -> Box<dyn VMConnector> {
        match config {
            VMConfig::Local { .. } => {
                let connector = LocalVMConnector { config };
                Box::new(connector)
            }
            VMConfig::Azure { .. } => {
                let connector = AzureVMConnector { config };
                Box::new(connector)
            }
            VMConfig::Remote { host, port, api_key } => {
                // Create a remote connector that uses our HTTP API
                let connector = RemoteHttpConnector::new(host, port, api_key);
                Box::new(connector)
            }
        }
    }

    pub fn create_from_env() -> Result<Box<dyn VMConnector>> {
        // Try to load configuration from environment variables
        if let Ok(vm_type) = std::env::var("VM_TYPE") {
            match vm_type.as_str() {
                "local" => {
                    let hypervisor = std::env::var("HYPERVISOR")
                        .unwrap_or_else(|_| "hyperv".to_string());
                    let vm_name = std::env::var("VM_NAME")
                        .context("VM_NAME environment variable required")?;

                    let hypervisor_type = match hypervisor.as_str() {
                        "hyperv" => HypervisorType::HyperV,
                        "vmware" => HypervisorType::VMware,
                        "virtualbox" => HypervisorType::VirtualBox,
                        _ => HypervisorType::HyperV,
                    };

                    Ok(Box::new(LocalVMConnector::new(hypervisor_type, vm_name)))
                }
                "azure" => {
                    let subscription_id = std::env::var("AZURE_SUBSCRIPTION_ID")
                        .context("AZURE_SUBSCRIPTION_ID required")?;
                    let resource_group = std::env::var("AZURE_RESOURCE_GROUP")
                        .context("AZURE_RESOURCE_GROUP required")?;
                    let vm_name = std::env::var("AZURE_VM_NAME")
                        .context("AZURE_VM_NAME required")?;

                    Ok(Box::new(AzureVMConnector::new(
                        subscription_id,
                        resource_group,
                        vm_name,
                    )))
                }
                "remote" => {
                    let host = std::env::var("REMOTE_HOST")
                        .context("REMOTE_HOST required")?;
                    let port = std::env::var("REMOTE_PORT")
                        .unwrap_or_else(|_| "8080".to_string())
                        .parse()?;
                    let api_key = std::env::var("REMOTE_API_KEY").ok();

                    Ok(Box::new(RemoteHttpConnector::new(host, port, api_key)))
                }
                _ => Err(anyhow::anyhow!("Unknown VM_TYPE: {}", vm_type)),
            }
        } else {
            Err(anyhow::anyhow!("VM_TYPE environment variable not set"))
        }
    }
}

/// Remote HTTP connector for connecting to remote UI automation agents
struct RemoteHttpConnector {
    host: String,
    port: u16,
    api_key: Option<String>,
}

impl RemoteHttpConnector {
    fn new(host: String, port: u16, api_key: Option<String>) -> Self {
        Self { host, port, api_key }
    }
}

#[async_trait]
impl VMConnector for RemoteHttpConnector {
    async fn connect(&self) -> Result<VMConnectionInfo> {
        let connection_string = format!("http://{}:{}", self.host, self.port);

        // Test connection
        let client = reqwest::Client::new();
        let mut url = format!("{}/health", connection_string);

        if let Some(api_key) = &self.api_key {
            url = format!("{}?api_key={}", url, api_key);
        }

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to connect to remote agent"));
        }

        Ok(VMConnectionInfo {
            vm_name: self.host.clone(),
            status: VMStatus::Running,
            connection_string,
            metadata: HashMap::new(),
        })
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn get_status(&self) -> Result<VMStatus> {
        // Check if the remote agent is responding
        match self.connect().await {
            Ok(_) => Ok(VMStatus::Running),
            Err(_) => Ok(VMStatus::Stopped),
        }
    }

    async fn start_vm(&self) -> Result<()> {
        // Not applicable for remote HTTP connector
        Ok(())
    }

    async fn stop_vm(&self) -> Result<()> {
        // Not applicable for remote HTTP connector
        Ok(())
    }

    async fn restart_vm(&self) -> Result<()> {
        // Not applicable for remote HTTP connector
        Ok(())
    }

    async fn deploy_agent(&self, _agent_path: &str) -> Result<()> {
        // Agent should already be deployed for remote HTTP connector
        Ok(())
    }
}