# Element Function Benchmarks

This directory contains comprehensive benchmarks for testing the performance of UI element tree fetching operations in the terminator library.

## Benchmark Files

### 1. `element_tree_benchmarks.rs`
Comprehensive benchmarks for individual element functions and operations:

- **Element Attributes**: `get_attributes()`, `get_role()`, `get_name()`, `get_id()`
- **Element Bounds**: `get_bounds()` performance
- **Element Children**: Tree traversal at different depths (1-3 levels)
- **Element Parent**: Parent element retrieval
- **Element Text**: Text extraction at different depths (1-3 levels)
- **Element State**: `is_enabled()`, `is_visible()`, `is_focused()`, `is_keyboard_focusable()`
- **Tree Traversal**: Breadth-first and depth-first traversal strategies
- **Serialization**: Converting elements to serializable format and JSON
- **Convenience Methods**: `id_or_empty()`, `name_or_empty()`, etc.
- **Bulk Operations**: Performance with 5, 10, and 20 elements

### 2. `tree_performance_benchmarks.rs`
Specialized benchmarks focused on tree structure operations:

- **Tree Depth Analysis**: Measuring maximum depth and counting nodes by level
- **Traversal Strategies**: Comparing different tree traversal approaches with limits
- **Search Operations**: Finding elements by role, properties, and text
- **Tree Filtering**: Filtering visible elements and interactive elements
- **Tree Transformations**: Converting to different data formats
- **Tree Statistics**: Gathering comprehensive tree metrics
- **Memory Efficiency**: Testing lazy iteration vs. bulk collection

## Running the Benchmarks

### Prerequisites
- Ensure you have a desktop environment available (the benchmarks require UI automation)
- On Windows: Make sure UI Automation is available
- On macOS: Ensure accessibility permissions are granted
- On Linux: Ensure accessibility APIs are available

### Run All Benchmarks
```bash
cd terminator
cargo bench
```

### Run Specific Benchmark Suite
```bash
# Run only element function benchmarks
cargo bench --bench element_tree_benchmarks

# Run only tree performance benchmarks
cargo bench --bench tree_performance_benchmarks
```

### Run Specific Benchmark Groups
```bash
# Run only attribute-related benchmarks
cargo bench --bench element_tree_benchmarks element_attributes

# Run only tree traversal benchmarks
cargo bench --bench tree_performance_benchmarks tree_traversal_strategies
```

### Generate HTML Reports
The benchmarks are configured to generate HTML reports with detailed graphs:

```bash
cargo bench
# Reports will be generated in target/criterion/
```

Open `target/criterion/report/index.html` in your browser to view detailed performance reports.

## Benchmark Categories

### Performance Focus Areas

1. **Individual Element Operations**
   - Attribute retrieval
   - Property access
   - State queries

2. **Tree Navigation**
   - Parent/child relationships
   - Depth-first vs breadth-first traversal
   - Limited vs unlimited traversal

3. **Data Extraction**
   - Text content retrieval
   - Bounds calculation
   - Serialization performance

4. **Search and Filtering**
   - Role-based filtering
   - Property-based search
   - Visibility filtering

5. **Memory Efficiency**
   - Lazy vs eager evaluation
   - Minimal data collection
   - Bulk operations

### Understanding Results

The benchmarks measure:
- **Throughput**: Operations per second
- **Latency**: Time per operation
- **Memory Usage**: Indirect measurement through operation patterns
- **Scalability**: Performance across different tree sizes and depths

### Performance Optimization Tips

Based on benchmark results, consider:

1. **Use lazy evaluation** when possible (process elements as you find them)
2. **Limit tree depth** to prevent exponential explosion
3. **Batch operations** when working with multiple elements
4. **Filter early** to reduce the number of elements processed
5. **Use minimal data extraction** when full element data isn't needed

## Customizing Benchmarks

### Adding New Benchmarks

1. Add your benchmark function to the appropriate file
2. Include it in the `criterion_group!` macro
3. Use `black_box()` to prevent compiler optimizations
4. Set reasonable limits to prevent infinite loops

### Benchmark Configuration

Modify these parameters in the benchmark code:
- `max_depth`: Control tree traversal depth
- `node_limit`: Limit number of elements processed
- `child_limit`: Limit children per element (use `.take(N)`)
- `timeout`: Set timeouts for operations

### Environment Variables

Set these environment variables to control benchmark behavior:
```bash
# Run benchmarks faster (less precision)
export CRITERION_DEBUG=1

# Set custom benchmark duration
export CRITERION_MEASUREMENT_TIME=10
```

## Troubleshooting

### Common Issues

1. **"Could not initialize UI automation"**
   - Ensure desktop environment is available
   - Check accessibility permissions
   - Try running with administrator/root privileges

2. **Benchmarks timeout or hang**
   - Reduce node limits in benchmark code
   - Decrease traversal depth limits
   - Check for infinite loops in tree structure

3. **Inconsistent results**
   - Close unnecessary applications
   - Run benchmarks multiple times
   - Ensure stable desktop environment

### Platform-Specific Notes

**Windows:**
- Requires UI Automation framework
- May need administrator privileges
- Performance varies with Windows version

**macOS:**
- Requires accessibility permissions
- Grant permissions in System Preferences > Security & Privacy > Accessibility
- Performance depends on accessibility API version

**Linux:**
- Requires AT-SPI or similar accessibility framework
- Install accessibility packages if needed
- Performance varies by desktop environment

## Interpreting Results

### Key Metrics

- **Mean time**: Average execution time
- **Standard deviation**: Consistency of performance
- **Throughput**: Operations per second
- **Change %**: Comparison with previous runs

### Performance Baselines

Typical performance ranges (varies by system):
- **Element attribute access**: 1-10 μs
- **Tree traversal (100 nodes)**: 1-50 ms
- **Serialization (single element)**: 10-100 μs
- **Search operations**: 10-500 ms (depends on tree size)

### Optimization Opportunities

Look for:
- High standard deviation (inconsistent performance)
- Linear scaling issues (O(n²) instead of O(n))
- Memory allocation patterns
- API call frequency 