// Workflow format detection - YAML vs TypeScript

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowFormat {
    Yaml,
    TypeScript,
}

/// Detect workflow format from URL
pub fn detect_workflow_format(url: &str) -> WorkflowFormat {
    // Handle file:// URLs
    if url.starts_with("file://") {
        let path_str = url.strip_prefix("file://").unwrap_or(url);
        let path = Path::new(path_str);

        // Check if it's a directory
        if path.is_dir() {
            // Look for package.json AND terminator.ts/workflow.ts/index.ts
            let package_json = path.join("package.json");
            let terminator_ts = path.join("terminator.ts");
            let workflow_ts = path.join("workflow.ts");
            let index_ts = path.join("index.ts");

            // Also check src/ subfolder for TypeScript files
            let src_terminator_ts = path.join("src").join("terminator.ts");
            let src_workflow_ts = path.join("src").join("workflow.ts");
            let src_index_ts = path.join("src").join("index.ts");

            if package_json.exists()
                && (terminator_ts.exists()
                    || workflow_ts.exists()
                    || index_ts.exists()
                    || src_terminator_ts.exists()
                    || src_workflow_ts.exists()
                    || src_index_ts.exists())
            {
                return WorkflowFormat::TypeScript;
            }
        } else if path.is_file() {
            // Check file extension
            if let Some(ext) = path.extension() {
                match ext.to_str() {
                    Some("ts") | Some("js") => return WorkflowFormat::TypeScript,
                    Some("yml") | Some("yaml") => return WorkflowFormat::Yaml,
                    _ => {}
                }
            }
        }
    }

    // Default to YAML for backward compatibility (includes http/https URLs)
    WorkflowFormat::Yaml
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_yaml_file() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_file = temp_dir.path().join("workflow.yml");
        fs::write(&yaml_file, "steps: []").unwrap();

        let url = format!("file://{}", yaml_file.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::Yaml);
    }

    #[test]
    fn test_detect_yaml_file_yaml_extension() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_file = temp_dir.path().join("workflow.yaml");
        fs::write(&yaml_file, "steps: []").unwrap();

        let url = format!("file://{}", yaml_file.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::Yaml);
    }

    #[test]
    fn test_detect_ts_file() {
        let temp_dir = TempDir::new().unwrap();
        let ts_file = temp_dir.path().join("workflow.ts");
        fs::write(&ts_file, "export default {};").unwrap();

        let url = format!("file://{}", ts_file.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_js_file() {
        let temp_dir = TempDir::new().unwrap();
        let js_file = temp_dir.path().join("workflow.js");
        fs::write(&js_file, "export default {};").unwrap();

        let url = format!("file://{}", js_file.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_ts_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir).unwrap();
        fs::write(project_dir.join("package.json"), "{}").unwrap();
        fs::write(project_dir.join("workflow.ts"), "export default {};").unwrap();

        let url = format!("file://{}", project_dir.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_ts_project_with_index() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir).unwrap();
        fs::write(project_dir.join("package.json"), "{}").unwrap();
        fs::write(project_dir.join("index.ts"), "export default {};").unwrap();

        let url = format!("file://{}", project_dir.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_ts_project_with_src_folder() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir).unwrap();
        fs::create_dir(project_dir.join("src")).unwrap();
        fs::write(project_dir.join("package.json"), "{}").unwrap();
        fs::write(project_dir.join("src").join("terminator.ts"), "export default {};").unwrap();

        let url = format!("file://{}", project_dir.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_ts_project_with_src_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir).unwrap();
        fs::create_dir(project_dir.join("src")).unwrap();
        fs::write(project_dir.join("package.json"), "{}").unwrap();
        fs::write(project_dir.join("src").join("workflow.ts"), "export default {};").unwrap();

        let url = format!("file://{}", project_dir.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_ts_project_with_src_index() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir).unwrap();
        fs::create_dir(project_dir.join("src")).unwrap();
        fs::write(project_dir.join("package.json"), "{}").unwrap();
        fs::write(project_dir.join("src").join("index.ts"), "export default {};").unwrap();

        let url = format!("file://{}", project_dir.display());
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::TypeScript);
    }

    #[test]
    fn test_detect_directory_without_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir).unwrap();
        fs::write(project_dir.join("workflow.ts"), "export default {};").unwrap();

        let url = format!("file://{}", project_dir.display());
        // Should default to YAML if no package.json
        assert_eq!(detect_workflow_format(&url), WorkflowFormat::Yaml);
    }

    #[test]
    fn test_http_url_defaults_to_yaml() {
        assert_eq!(
            detect_workflow_format("https://example.com/workflow.yml"),
            WorkflowFormat::Yaml
        );
        assert_eq!(
            detect_workflow_format("http://example.com/workflow.yaml"),
            WorkflowFormat::Yaml
        );
    }
}