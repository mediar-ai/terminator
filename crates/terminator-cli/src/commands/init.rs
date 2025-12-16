use anyhow::{Context, Result};
use clap::Args;
use colored::*;
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

#[derive(Debug, Args)]
pub struct InitCommand {
    /// Name of the workflow project to create
    #[arg(default_value = "my-workflow")]
    name: String,

    /// Skip npm install after scaffolding
    #[arg(long)]
    skip_install: bool,

    /// Use bun instead of npm for installation
    #[arg(long)]
    use_bun: bool,
}

impl InitCommand {
    pub async fn execute(&self) -> Result<()> {
        let project_path = Path::new(&self.name);

        // Check if directory already exists
        if project_path.exists() {
            return Err(anyhow::anyhow!(
                "Directory '{}' already exists. Please choose a different name or remove the existing directory.",
                self.name
            ));
        }

        println!("{}", "🚀 Creating new Terminator workflow...".bold().cyan());
        println!();

        // Create directory structure
        self.create_directory_structure(project_path)?;

        // Create all template files
        self.create_package_json(project_path)?;
        self.create_tsconfig(project_path)?;
        self.create_main_workflow(project_path)?;
        self.create_example_step(project_path)?;
        self.create_gitignore(project_path)?;

        println!("  {} Created project structure", "✓".green());

        // Install dependencies
        if !self.skip_install {
            self.install_dependencies(project_path)?;
        }

        // Print success message
        self.print_success_message();

        Ok(())
    }

    fn create_directory_structure(&self, project_path: &Path) -> Result<()> {
        fs::create_dir_all(project_path.join("src/steps"))
            .context("Failed to create directory structure")?;
        Ok(())
    }

    fn create_package_json(&self, project_path: &Path) -> Result<()> {
        let package_json = format!(
            r#"{{
  "name": "{}",
  "version": "1.0.0",
  "description": "Terminator workflow automation",
  "main": "src/terminator.ts",
  "scripts": {{
    "build": "tsc --noEmit"
  }},
  "dependencies": {{
    "@mediar-ai/workflow": "latest"
  }},
  "devDependencies": {{
    "@types/node": "^20.0.0",
    "typescript": "^5.0.0"
  }}
}}
"#,
            self.name
        );

        fs::write(project_path.join("package.json"), package_json)
            .context("Failed to create package.json")?;
        Ok(())
    }

    fn create_tsconfig(&self, project_path: &Path) -> Result<()> {
        let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": false,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
"#;

        fs::write(project_path.join("tsconfig.json"), tsconfig)
            .context("Failed to create tsconfig.json")?;
        Ok(())
    }

    fn create_main_workflow(&self, project_path: &Path) -> Result<()> {
        let workflow = r#"import { createWorkflow, z } from "@mediar-ai/workflow";
import { exampleStep } from "./steps/01-example-step";

export default createWorkflow({
  input: z.object({}).optional(),
  steps: [
    exampleStep,
    // Add more steps here
  ],
  onError: async ({ error }) => {
    console.error("Workflow failed");
    console.error(`Error: ${error.message}`);
  },
});
"#;

        fs::write(project_path.join("src/terminator.ts"), workflow)
            .context("Failed to create src/terminator.ts")?;
        Ok(())
    }

    fn create_example_step(&self, project_path: &Path) -> Result<()> {
        let step = r#"import { createStep } from "@mediar-ai/workflow";

export const exampleStep = createStep({
  id: "example_step",
  name: "Example Step",
  execute: async ({ desktop }) => {
    console.log("Starting example step...");

    // Example: Open an application
    // const app = desktop.openApplication("notepad");
    // await desktop.delay(1500);

    // Example: Find and click a button
    // const button = await desktop.locator("role:Button && name:OK").first(2000);
    // await button.click();

    // Example: Type text
    // await desktop.type("Hello World!");

    console.log("Example step completed!");

    return {
      state: {
        completed: true,
      },
    };
  },
});
"#;

        fs::write(project_path.join("src/steps/01-example-step.ts"), step)
            .context("Failed to create src/steps/01-example-step.ts")?;
        Ok(())
    }

    fn create_gitignore(&self, project_path: &Path) -> Result<()> {
        let gitignore = r#"node_modules/
dist/
*.log
.DS_Store
"#;

        fs::write(project_path.join(".gitignore"), gitignore)
            .context("Failed to create .gitignore")?;
        Ok(())
    }

    fn install_dependencies(&self, project_path: &Path) -> Result<()> {
        println!();
        println!("  {} Installing dependencies...", "📦".cyan());

        let (cmd, args) = if self.use_bun {
            ("bun", vec!["install"])
        } else {
            ("npm", vec!["install"])
        };

        let result = ProcessCommand::new(cmd)
            .args(&args)
            .current_dir(project_path)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                println!("  {} Dependencies installed", "✓".green());
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!(
                    "  {} Failed to install dependencies: {}",
                    "⚠".yellow(),
                    stderr
                );
                println!("  Run `cd {} && npm install` manually", self.name);
                Ok(())
            }
            Err(e) => {
                println!("  {} Could not run {}: {}", "⚠".yellow(), cmd, e);
                println!("  Run `cd {} && npm install` manually", self.name);
                Ok(())
            }
        }
    }

    fn print_success_message(&self) {
        println!();
        println!("{}", "✅ Workflow created successfully!".bold().green());
        println!();
        println!("Next steps:");
        println!("  1. cd {}", self.name.cyan());
        println!("  2. npm install");
        println!("  3. Edit {} to add your steps", "src/steps/".cyan());
        println!("  4. Run your workflow:");
        println!(
            "     {}",
            format!("terminator mcp run {}/src/terminator.ts", self.name).cyan()
        );
        println!();
        println!(
            "Documentation: {}",
            "https://github.com/mediar-ai/terminator".underline()
        );
    }
}
