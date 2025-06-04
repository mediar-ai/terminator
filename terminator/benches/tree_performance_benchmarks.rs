use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::VecDeque;
use terminator::{Desktop, UIElement};
use tokio::runtime::Runtime;

/// Helper struct for tree benchmarking
struct TreeBenchmarkData {
    root_element: UIElement,
}

impl TreeBenchmarkData {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let desktop = Desktop::new(false, false)?;
        let root_element = desktop.root();
        
        Ok(Self {
            root_element,
        })
    }
}

/// Benchmark tree depth measurements
fn bench_tree_depth_analysis(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_depth_analysis");
    
    // Benchmark measuring tree depth from root
    group.bench_function("measure_max_depth_from_root", |b| {
        b.iter(|| {
            fn max_depth(element: &UIElement, current_depth: usize, max_allowed: usize) -> usize {
                if current_depth >= max_allowed {
                    return current_depth;
                }
                
                if let Ok(children) = element.children() {
                    children.iter()
                        .take(10) // Limit children to prevent explosion
                        .map(|child| max_depth(child, current_depth + 1, max_allowed))
                        .max()
                        .unwrap_or(current_depth)
                } else {
                    current_depth
                }
            }
            
            black_box(max_depth(&data.root_element, 0, 5))
        })
    });
    
    // Benchmark counting nodes at each depth level
    group.bench_function("count_nodes_by_depth", |b| {
        b.iter(|| {
            let mut depth_counts = vec![0; 6]; // Track up to depth 5
            let mut queue = VecDeque::new();
            queue.push_back((data.root_element.clone(), 0));
            
            while let Some((element, depth)) = queue.pop_front() {
                if depth >= depth_counts.len() {
                    continue;
                }
                
                depth_counts[depth] += 1;
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(10) {
                        if depth + 1 < depth_counts.len() {
                            queue.push_back((child, depth + 1));
                        }
                    }
                }
            }
            
            black_box(depth_counts)
        })
    });
    
    group.finish();
}

/// Benchmark different tree traversal strategies
fn bench_tree_traversal_strategies(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_traversal_strategies");
    
    // Breadth-first with node limit
    for node_limit in [50, 100, 200] {
        group.bench_with_input(
            BenchmarkId::new("breadth_first_limited", node_limit),
            &node_limit,
            |b, &limit| {
                b.iter(|| {
                    let mut visited = 0;
                    let mut queue = VecDeque::new();
                    queue.push_back(data.root_element.clone());
                    
                    while let Some(element) = queue.pop_front() {
                        visited += 1;
                        if visited >= limit {
                            break;
                        }
                        
                        if let Ok(children) = element.children() {
                            for child in children.into_iter().take(5) {
                                queue.push_back(child);
                            }
                        }
                    }
                    
                    black_box(visited)
                })
            },
        );
    }
    
    // Depth-first with depth limit
    for depth_limit in [3, 4, 5] {
        group.bench_with_input(
            BenchmarkId::new("depth_first_depth_limited", depth_limit),
            &depth_limit,
            |b, &limit| {
                b.iter(|| {
                    let mut visited = 0;
                    let mut stack = vec![(data.root_element.clone(), 0)];
                    
                    while let Some((element, depth)) = stack.pop() {
                        if depth > limit {
                            continue;
                        }
                        
                        visited += 1;
                        
                        if let Ok(children) = element.children() {
                            for child in children.into_iter().take(5) {
                                stack.push((child, depth + 1));
                            }
                        }
                    }
                    
                    black_box(visited)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark tree search operations
fn bench_tree_search_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_search_operations");
    
    // Find elements by role
    group.bench_function("find_elements_by_role", |b| {
        b.iter(|| {
            let target_role = "Button";
            let mut found_elements = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            let mut visited = 0;
            
            while let Some(element) = queue.pop_front() {
                visited += 1;
                if visited > 100 { // Limit search scope
                    break;
                }
                
                if element.role() == target_role {
                    found_elements.push(element.clone());
                }
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(5) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(found_elements.len())
        })
    });
    
    // Find elements with specific properties
    group.bench_function("find_elements_with_text", |b| {
        b.iter(|| {
            let mut found_elements = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            let mut visited = 0;
            
            while let Some(element) = queue.pop_front() {
                visited += 1;
                if visited > 100 { // Limit search scope
                    break;
                }
                
                if element.name().is_some() || element.attributes().value.is_some() {
                    found_elements.push(element.clone());
                }
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(5) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(found_elements.len())
        })
    });
    
    group.finish();
}

/// Benchmark tree filtering operations
fn bench_tree_filtering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_filtering");
    
    // Filter visible elements
    group.bench_function("filter_visible_elements", |b| {
        b.iter(|| {
            let mut visible_elements = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            let mut visited = 0;
            
            while let Some(element) = queue.pop_front() {
                visited += 1;
                if visited > 100 { // Limit search scope
                    break;
                }
                
                if element.is_visible().unwrap_or(false) {
                    visible_elements.push(element.clone());
                }
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(5) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(visible_elements.len())
        })
    });
    
    // Filter interactive elements
    group.bench_function("filter_interactive_elements", |b| {
        b.iter(|| {
            let mut interactive_elements = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            let mut visited = 0;
            
            while let Some(element) = queue.pop_front() {
                visited += 1;
                if visited > 100 { // Limit search scope
                    break;
                }
                
                let role = element.role();
                if matches!(role.as_str(), "Button" | "TextField" | "MenuItem" | "CheckBox" | "RadioButton") {
                    interactive_elements.push(element.clone());
                }
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(5) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(interactive_elements.len())
        })
    });
    
    group.finish();
}

/// Benchmark tree transformation operations
fn bench_tree_transformations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_transformations");
    
    // Transform tree to serializable format
    group.bench_function("transform_to_serializable", |b| {
        b.iter(|| {
            let mut serializable_elements = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            let mut visited = 0;
            
            while let Some(element) = queue.pop_front() {
                visited += 1;
                if visited > 50 { // Limit transformation scope
                    break;
                }
                
                serializable_elements.push(element.to_serializable());
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(3) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(serializable_elements)
        })
    });
    
    // Build element hierarchy map
    group.bench_function("build_hierarchy_map", |b| {
        b.iter(|| {
            let mut hierarchy_map = std::collections::HashMap::new();
            let mut queue = VecDeque::new();
            queue.push_back((data.root_element.clone(), None));
            let mut visited = 0;
            
            while let Some((element, parent_id)) = queue.pop_front() {
                visited += 1;
                if visited > 50 { // Limit scope
                    break;
                }
                
                let element_id = element.id_or_empty();
                if !element_id.is_empty() {
                    hierarchy_map.insert(element_id.clone(), parent_id.clone());
                }
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(3) {
                        queue.push_back((child, Some(element_id.clone())));
                    }
                }
            }
            
            black_box(hierarchy_map)
        })
    });
    
    group.finish();
}

/// Benchmark tree statistics gathering
fn bench_tree_statistics(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_statistics");
    
    // Gather comprehensive tree stats
    group.bench_function("gather_tree_statistics", |b| {
        b.iter(|| {
            let mut stats = std::collections::HashMap::new();
            let mut total_nodes = 0;
            let mut role_counts = std::collections::HashMap::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            
            while let Some(element) = queue.pop_front() {
                total_nodes += 1;
                if total_nodes > 100 { // Limit scope
                    break;
                }
                
                let role = element.role();
                *role_counts.entry(role).or_insert(0) += 1;
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(5) {
                        queue.push_back(child);
                    }
                }
            }
            
            stats.insert("total_nodes".to_string(), total_nodes);
            stats.insert("unique_roles".to_string(), role_counts.len());
            
            black_box((stats, role_counts))
        })
    });
    
    group.finish();
}

/// Benchmark memory efficiency of tree operations
fn bench_tree_memory_efficiency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        TreeBenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_memory_efficiency");
    
    // Test memory usage with different collection strategies
    group.bench_function("lazy_iteration", |b| {
        b.iter(|| {
            let mut count = 0;
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            
            // Process without storing all elements
            while let Some(element) = queue.pop_front() {
                count += 1;
                if count > 50 {
                    break;
                }
                
                // Process element immediately without storing
                let _attrs = element.attributes();
                let _bounds = element.bounds();
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(3) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(count)
        })
    });
    
    // Test with minimal data collection
    group.bench_function("minimal_data_collection", |b| {
        b.iter(|| {
            let mut essential_data = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(data.root_element.clone());
            let mut visited = 0;
            
            while let Some(element) = queue.pop_front() {
                visited += 1;
                if visited > 50 {
                    break;
                }
                
                // Store only essential data
                essential_data.push((
                    element.role(),
                    element.id_or_empty(),
                    element.bounds().ok(),
                ));
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(3) {
                        queue.push_back(child);
                    }
                }
            }
            
            black_box(essential_data)
        })
    });
    
    group.finish();
}

criterion_group!(
    tree_benches,
    bench_tree_depth_analysis,
    bench_tree_traversal_strategies,
    bench_tree_search_operations,
    bench_tree_filtering,
    bench_tree_transformations,
    bench_tree_statistics,
    bench_tree_memory_efficiency
);

criterion_main!(tree_benches); 