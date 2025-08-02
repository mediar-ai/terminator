use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "terminator")]
#[command(about = "🤖 Terminator CLI - AI-native GUI automation")]
#[command(
    long_about = "Terminator CLI provides tools for managing the Terminator project, including version management, releases, and development workflows."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
#[clap(rename_all = "lower")]
pub enum BumpLevel {
    #[default]
    Patch,
    Minor,
    Major,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
#[clap(rename_all = "lower")]
pub enum AIProvider {
    #[default]
    Anthropic,
    OpenAI,
    Gemini,
}

impl std::fmt::Display for BumpLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

#[derive(Parser, Debug)]
pub struct ReleaseArgs {
    /// The part of the version to bump: patch, minor, or major.
    #[clap(value_enum, default_value_t = BumpLevel::Patch)]
    pub level: BumpLevel,
}

#[derive(Parser, Debug)]
pub struct McpChatArgs {
    /// MCP server URL (e.g., http://localhost:3000)
    #[clap(long, short = 'u', conflicts_with = "command")]
    pub url: Option<String>,

    /// Command to start MCP server via stdio (e.g., "npx -y terminator-mcp-agent")
    #[clap(long, short = 'c', conflicts_with = "url")]
    pub command: Option<String>,

    /// Specify AIProvider 
    #[clap(long, short = 'a', default_value_t = AIProvider::Anthropic, value_enum)]
    pub aiprovider: AIProvider,

}

#[derive(Parser, Debug)]
pub struct McpExecArgs {
    /// MCP server URL
    #[clap(long, short = 'u', conflicts_with = "command")]
    pub url: Option<String>,

    /// Command to start MCP server via stdio
    #[clap(long, short = 'c', conflicts_with = "url")]
    pub command: Option<String>,

    /// Tool name to execute
    pub tool: String,

    /// Arguments for the tool (as JSON or simple string)
    pub args: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum InputType {
    Auto,
    Gist,
    Raw,
    File,
}

#[derive(Parser, Debug)]
pub struct McpRunArgs {
    /// MCP server URL (e.g., http://localhost:3000)
    #[clap(long, short = 'u', conflicts_with = "command")]
    pub url: Option<String>,

    /// Command to start MCP server via stdio (e.g., "npx -y terminator-mcp-agent")
    #[clap(long, short = 'c', conflicts_with = "url")]
    pub command: Option<String>,

    /// Input source - can be a GitHub gist URL, raw gist URL, or local file path (JSON/YAML)
    pub input: String,

    /// Input type (auto-detected by default)
    #[clap(long, value_enum, default_value = "auto")]
    pub input_type: InputType,

    /// Dry run - parse and validate the workflow without executing
    #[clap(long)]
    pub dry_run: bool,

    /// Verbose output
    #[clap(long, short)]
    pub verbose: bool,

    /// Stop on first error (default: true)
    #[clap(long)]
    pub no_stop_on_error: bool,

    /// Include detailed results (default: true)
    #[clap(long)]
    pub no_detailed_results: bool,
}

#[derive(Subcommand)]
pub enum McpCommands {
    /// Interactive chat with MCP server
    Chat(McpChatArgs),
    /// Interactive AI-powered chat with MCP server
    AiChat(McpChatArgs),
    /// Execute a single MCP tool
    Exec(McpExecArgs),
    /// Execute a workflow sequence from a local file or GitHub gist
    Run(McpRunArgs),
}

#[derive(Subcommand)]
pub enum Commands {
    /// Bump patch version (x.y.Z+1)
    Patch,
    /// Bump minor version (x.Y+1.0)
    Minor,
    /// Bump major version (X+1.0.0)
    Major,
    /// Sync all package versions without bumping
    Sync,
    /// Show current version status
    Status,
    /// Tag current version and push (triggers CI)
    Tag,
    /// Full release: bump version + tag + push
    Release(ReleaseArgs),
    /// MCP client commands
    #[command(subcommand)]
    Mcp(McpCommands),
}
