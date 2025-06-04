use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use terminator::{Desktop, UIElement};
use tokio::runtime::Runtime;

/// Helper struct to hold benchmark data
struct BenchmarkData {
    root_element: UIElement,
    sample_elements: Vec<UIElement>,
}

impl BenchmarkData {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let desktop = Desktop::new(false, false)?;
        let root_element = desktop.root();
        
        // Collect some sample elements for benchmarking
        let mut sample_elements = Vec::new();
        if let Ok(children) = root_element.children() {
            sample_elements.extend(children.into_iter().take(10));
        }
        
        Ok(Self {
            root_element,
            sample_elements,
        })
    }
}

/// Benchmark getting element attributes
fn bench_element_attributes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            // Fallback for environments without UI access
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_attributes");
    
    for element in &data.sample_elements {
        group.bench_function("get_attributes", |b| {
            b.iter(|| {
                black_box(element.attributes())
            })
        });
        
        group.bench_function("get_role", |b| {
            b.iter(|| {
                black_box(element.role())
            })
        });
        
        group.bench_function("get_name", |b| {
            b.iter(|| {
                black_box(element.name())
            })
        });
        
        group.bench_function("get_id", |b| {
            b.iter(|| {
                black_box(element.id())
            })
        });
        
        // Only benchmark first element to avoid too many iterations
        break;
    }
    
    group.finish();
}

/// Benchmark getting element bounds
fn bench_element_bounds(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_bounds");
    
    for element in &data.sample_elements {
        group.bench_function("get_bounds", |b| {
            b.iter(|| {
                black_box(element.bounds())
            })
        });
        
        // Only benchmark first element to avoid too many iterations
        break;
    }
    
    group.finish();
}

/// Benchmark getting element children
fn bench_element_children(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_children");
    
    // Benchmark children retrieval at different depths
    for depth in [1, 2, 3] {
        group.bench_with_input(
            BenchmarkId::new("get_children_depth", depth),
            &depth,
            |b, &depth| {
                let current_elements = vec![data.root_element.clone()];
                
                b.iter(|| {
                    let mut result_count = 0;
                    let mut elements_to_process = current_elements.clone();
                    
                    for _ in 0..depth {
                        let mut next_elements = Vec::new();
                        for element in &elements_to_process {
                            if let Ok(children) = element.children() {
                                result_count += children.len();
                                next_elements.extend(children.into_iter().take(5)); // Limit to prevent explosion
                            }
                        }
                        elements_to_process = next_elements;
                        if elements_to_process.is_empty() {
                            break;
                        }
                    }
                    black_box(result_count)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark getting element parent
fn bench_element_parent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_parent");
    
    for element in &data.sample_elements {
        group.bench_function("get_parent", |b| {
            b.iter(|| {
                black_box(element.parent())
            })
        });
        
        // Only benchmark first element to avoid too many iterations
        break;
    }
    
    group.finish();
}

/// Benchmark getting element text content
fn bench_element_text(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_text");
    
    // Test different text retrieval depths
    for depth in [1, 2, 3] {
        for element in &data.sample_elements {
            group.bench_with_input(
                BenchmarkId::new("get_text_depth", depth),
                &depth,
                |b, &depth| {
                    b.iter(|| {
                        black_box(element.text(depth))
                    })
                },
            );
            
            // Only benchmark first element to avoid too many iterations
            break;
        }
    }
    
    group.finish();
}

/// Benchmark element state queries
fn bench_element_state(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_state");
    
    for element in &data.sample_elements {
        group.bench_function("is_enabled", |b| {
            b.iter(|| {
                black_box(element.is_enabled())
            })
        });
        
        group.bench_function("is_visible", |b| {
            b.iter(|| {
                black_box(element.is_visible())
            })
        });
        
        group.bench_function("is_focused", |b| {
            b.iter(|| {
                black_box(element.is_focused())
            })
        });
        
        group.bench_function("is_keyboard_focusable", |b| {
            b.iter(|| {
                black_box(element.is_keyboard_focusable())
            })
        });
        
        // Only benchmark first element to avoid too many iterations
        break;
    }
    
    group.finish();
}

/// Benchmark tree traversal operations
fn bench_tree_traversal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("tree_traversal");
    
    // Benchmark breadth-first traversal
    group.bench_function("breadth_first_traversal", |b| {
        b.iter(|| {
            let mut visited_count = 0;
            let mut queue = vec![data.root_element.clone()];
            let mut depth = 0;
            
            while !queue.is_empty() && depth < 3 {
                let mut next_queue = Vec::new();
                
                for element in queue {
                    visited_count += 1;
                    if let Ok(children) = element.children() {
                        next_queue.extend(children.into_iter().take(5)); // Limit children
                    }
                }
                
                queue = next_queue;
                depth += 1;
            }
            
            black_box(visited_count)
        })
    });
    
    // Benchmark depth-first traversal
    group.bench_function("depth_first_traversal", |b| {
        b.iter(|| {
            let mut visited_count = 0;
            let mut stack = vec![(data.root_element.clone(), 0)];
            
            while let Some((element, depth)) = stack.pop() {
                if depth >= 3 {
                    continue;
                }
                
                visited_count += 1;
                
                if let Ok(children) = element.children() {
                    for child in children.into_iter().take(5) {
                        stack.push((child, depth + 1));
                    }
                }
            }
            
            black_box(visited_count)
        })
    });
    
    group.finish();
}

/// Benchmark element serialization operations
fn bench_element_serialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("element_serialization");
    
    for element in &data.sample_elements {
        group.bench_function("to_serializable", |b| {
            b.iter(|| {
                black_box(element.to_serializable())
            })
        });
        
        group.bench_function("serialize_to_json", |b| {
            b.iter(|| {
                let serializable = element.to_serializable();
                black_box(serde_json::to_string(&serializable))
            })
        });
        
        // Only benchmark first element to avoid too many iterations
        break;
    }
    
    group.finish();
}

/// Benchmark convenience methods
fn bench_convenience_methods(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("convenience_methods");
    
    for element in &data.sample_elements {
        group.bench_function("id_or_empty", |b| {
            b.iter(|| {
                black_box(element.id_or_empty())
            })
        });
        
        group.bench_function("name_or_empty", |b| {
            b.iter(|| {
                black_box(element.name_or_empty())
            })
        });
        
        group.bench_function("value_or_empty", |b| {
            b.iter(|| {
                black_box(element.value_or_empty())
            })
        });
        
        group.bench_function("application_name", |b| {
            b.iter(|| {
                black_box(element.application_name())
            })
        });
        
        group.bench_function("window_title", |b| {
            b.iter(|| {
                black_box(element.window_title())
            })
        });
        
        // Only benchmark first element to avoid too many iterations
        break;
    }
    
    group.finish();
}

/// Benchmark bulk operations on multiple elements
fn bench_bulk_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async {
        BenchmarkData::new().await.unwrap_or_else(|_| {
            panic!("Could not initialize UI automation - ensure a desktop environment is available")
        })
    });

    let mut group = c.benchmark_group("bulk_operations");
    
    // Collect more elements for bulk testing
    let mut bulk_elements = data.sample_elements.clone();
    for element in &data.sample_elements {
        if let Ok(children) = element.children() {
            bulk_elements.extend(children.into_iter().take(5));
        }
    }
    
    for count in [5, 10, 20] {
        let elements_subset = bulk_elements.iter().take(count.min(bulk_elements.len())).collect::<Vec<_>>();
        
        group.bench_with_input(
            BenchmarkId::new("bulk_attributes", count),
            &elements_subset,
            |b, elements| {
                b.iter(|| {
                    let attributes: Vec<_> = elements.iter()
                        .map(|e| e.attributes())
                        .collect();
                    black_box(attributes)
                })
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("bulk_bounds", count),
            &elements_subset,
            |b, elements| {
                b.iter(|| {
                    let bounds: Vec<_> = elements.iter()
                        .map(|e| e.bounds())
                        .collect();
                    black_box(bounds)
                })
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("bulk_serialization", count),
            &elements_subset,
            |b, elements| {
                b.iter(|| {
                    let serialized: Vec<_> = elements.iter()
                        .map(|e| e.to_serializable())
                        .collect();
                    black_box(serialized)
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_element_attributes,
    bench_element_bounds,
    bench_element_children,
    bench_element_parent,
    bench_element_text,
    bench_element_state,
    bench_tree_traversal,
    bench_element_serialization,
    bench_convenience_methods,
    bench_bulk_operations
);

criterion_main!(benches); 