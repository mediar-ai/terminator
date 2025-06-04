use std::time::{Duration, Instant};
use terminator::Desktop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Background Cache Warming Demo");
    println!("================================");
    
    // Create desktop instance
    let desktop = Desktop::new(false, false)?;
    
    #[cfg(target_os = "windows")]
    {
        // Demo the performance difference
        demo_performance_difference(&desktop).await?;
        
        // Demo basic usage
        demo_basic_usage(&desktop).await?;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        println!("❌ Cache warming is currently only implemented for Windows");
        println!("   On other platforms, the feature returns UnsupportedOperation");
        
        match desktop.enable_background_cache_warmer(true, Some(30), Some(10)) {
            Ok(_) => println!("   Unexpected success!"),
            Err(e) => println!("   Expected error: {}", e),
        }
    }
    
    Ok(())
}

#[cfg(target_os = "windows")]
async fn demo_performance_difference(desktop: &Desktop) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 PERFORMANCE COMPARISON DEMO");
    println!("==============================");
    
    // Test without cache warming - measure actual UI tree building that gets cached
    println!("🔍 Testing UI tree building speed WITHOUT cache warming...");
    let start = Instant::now();
    let mut success_count = 0;
    let mut total_nodes = 0;
    
    for i in 0..3 {
        let attempt_start = Instant::now();
        let mut attempt_successful = false;
        
        // Test the ACTUAL operations that cache warming optimizes!
        let window_titles = ["Program Manager", "Windows Explorer", "Task Manager"];
        
        for title in &window_titles {
            if let Ok(tree) = desktop.get_window_tree_by_title(title) {
                attempt_successful = true;
                let node_count = count_ui_nodes(&tree);
                total_nodes += node_count;
                println!("   Attempt {}: ✅ Built '{}' UI tree ({} nodes) in {:?}", 
                         i + 1, title, node_count, attempt_start.elapsed());
                break;
            }
        }
        
        if attempt_successful {
            success_count += 1;
        } else {
            println!("   Attempt {}: ❌ Failed to build any UI tree in {:?}", 
                     i + 1, attempt_start.elapsed());
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    let without_cache_time = start.elapsed();
    println!("📈 Without cache: {} successes, {} total nodes in {:?} (avg: {:?})", 
             success_count, total_nodes, without_cache_time, 
             Duration::from_nanos(without_cache_time.as_nanos() as u64 / 3));
    
    // Enable cache warming
    println!("\n🔥 Enabling background cache warming...");
    desktop.enable_background_cache_warmer(true, Some(3), Some(8))?;
    
    println!("⏳ Waiting 8 seconds for cache to warm up...");
    tokio::time::sleep(Duration::from_secs(8)).await;
    
    // Test with cache warming - should be MUCH faster now!
    println!("🔍 Testing UI tree building speed WITH cache warming...");
    let start = Instant::now();
    success_count = 0;
    total_nodes = 0;
    
    for i in 0..3 {
        let attempt_start = Instant::now();
        let mut attempt_successful = false;
        
        // Test the same operations - these should be cached now!
        let window_titles = ["Program Manager", "Windows Explorer", "Task Manager"];
        
        for title in &window_titles {
            if let Ok(tree) = desktop.get_window_tree_by_title(title) {
                attempt_successful = true;
                let node_count = count_ui_nodes(&tree);
                total_nodes += node_count;
                let duration = attempt_start.elapsed();
                
                if duration.as_millis() < 50 {
                    println!("   Attempt {}: ⚡ INSTANT! Built '{}' UI tree ({} nodes) in {:?}", 
                             i + 1, title, node_count, duration);
                } else {
                    println!("   Attempt {}: ✅ Built '{}' UI tree ({} nodes) in {:?}", 
                             i + 1, title, node_count, duration);
                }
                break;
            }
        }
        
        if attempt_successful {
            success_count += 1;
        } else {
            println!("   Attempt {}: ❌ Failed to build any UI tree in {:?}", 
                     i + 1, attempt_start.elapsed());
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    let with_cache_time = start.elapsed();
    println!("📈 With cache: {} successes, {} total nodes in {:?} (avg: {:?})", 
             success_count, total_nodes, with_cache_time,
             Duration::from_nanos(with_cache_time.as_nanos() as u64 / 3));
    
    // Compare results
    if with_cache_time < without_cache_time {
        let improvement = ((without_cache_time.as_nanos() - with_cache_time.as_nanos()) as f64 
                          / without_cache_time.as_nanos() as f64) * 100.0;
        let speedup = without_cache_time.as_nanos() as f64 / with_cache_time.as_nanos() as f64;
        println!("🚀 RESULT: {:.1}% performance improvement with cache warming!", improvement);
        println!("🏃 {:.1}x speedup - UI trees are now cached!", speedup);
    } else {
        println!("⚠️  Cache warming didn't show improvement in this test");
    }
    
    // Disable cache warming
    desktop.enable_background_cache_warmer(false, None, None)?;
    
    Ok(())
}

// Helper function to count nodes in a UI tree
fn count_ui_nodes(node: &terminator::UINode) -> usize {
    let mut count = 1; // Count this node
    for child in &node.children {
        count += count_ui_nodes(child);
    }
    count
}

#[cfg(target_os = "windows")]
async fn demo_basic_usage(desktop: &Desktop) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎯 BASIC USAGE DEMO");
    println!("==================");
    
    // Check initial state
    println!("Initial cache warmer state: {}", 
             if desktop.is_cache_warmer_enabled() { "ENABLED" } else { "DISABLED" });
    
    // Enable with custom settings
    println!("🔄 Enabling cache warming with custom settings:");
    println!("   - Refresh interval: 15 seconds");
    println!("   - Max apps to cache: 6");
    
    desktop.enable_background_cache_warmer(true, Some(15), Some(6))?;
    println!("✅ Cache warming enabled!");
    
    // Let it run for a while
    for i in 1..=6 {
        println!("⏳ Running for {} seconds... (cache warmer: {})", 
                 i * 2, 
                 if desktop.is_cache_warmer_enabled() { "ACTIVE" } else { "INACTIVE" });
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    
    // Disable
    println!("🛑 Disabling cache warming...");
    desktop.enable_background_cache_warmer(false, None, None)?;
    println!("✅ Cache warming disabled!");
    
    println!("Final cache warmer state: {}", 
             if desktop.is_cache_warmer_enabled() { "ENABLED" } else { "DISABLED" });
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn demo_recommended_usage() {
    println!("\n💡 RECOMMENDED USAGE PATTERNS");
    println!("=============================");
    
    println!("🎯 For workflow recording applications:");
    println!("   desktop.enable_background_cache_warmer(true, Some(30), Some(10))?;");
    println!("   // 30 second intervals, cache up to 10 most recent apps");
    
    println!("\n🎯 For interactive automation:");
    println!("   desktop.enable_background_cache_warmer(true, Some(10), Some(5))?;");
    println!("   // 10 second intervals, cache up to 5 apps for faster response");
    
    println!("\n🎯 For resource-constrained systems:");
    println!("   desktop.enable_background_cache_warmer(true, Some(60), Some(3))?;");
    println!("   // 60 second intervals, cache only 3 apps to minimize resource usage");
    
    println!("\n⚠️  Remember to disable when done:");
    println!("   desktop.enable_background_cache_warmer(false, None, None)?;");
} 