const { Desktop, PropertyLoadingMode } = require('./index.js');

async function testGetWindowTree() {
    try {
        // Create desktop instance
        const desktop = new Desktop(false, false, 'info');
        
        // First, let's get a list of running applications to find a PID
        console.log('Getting list of applications...');
        const apps = desktop.applications();
        
        if (apps.length === 0) {
            console.log('No applications found');
            return;
        }
        
        // Let's use the first application
        const firstApp = apps[0];
        const pid = firstApp.processId();
        const appName = firstApp.attributes().name || 'Unknown App';
        
        console.log(`Testing with application: ${appName} (PID: ${pid})`);
        
        // Create a custom config for fast tree building
        const config = {
            propertyMode: PropertyLoadingMode.Fast,
            timeoutPerOperationMs: 50,
            yieldEveryNElements: 50,
            batchSize: 50
        };
        
        // Get the window tree with custom config
        console.log('Getting window tree with custom config...');
        const tree = desktop.getWindowTree(pid, null, config);
        
        // Function to display tree structure with limited depth
        function displayTree(node, depth = 0, maxDepth = 3) {
            const indent = '  '.repeat(depth);
            const attrs = node.attributes;
            const role = attrs.role;
            const name = attrs.name || '(no name)';
            
            console.log(`${indent}${role}: ${name}`);
            
            if (depth < maxDepth && node.children && node.children.length > 0) {
                console.log(`${indent}  └─ ${node.children.length} children:`);
                for (const child of node.children.slice(0, 5)) { // Limit to first 5 children
                    displayTree(child, depth + 1, maxDepth);
                }
                if (node.children.length > 5) {
                    console.log(`${indent}     ... and ${node.children.length - 5} more`);
                }
            }
        }
        
        console.log('\nWindow Tree Structure:');
        displayTree(tree);
        
        console.log(`\nTotal elements in tree: ${countElements(tree)}`);
        
        // Also test without config (should use defaults)
        console.log('\n--- Testing without config (defaults) ---');
        const defaultTree = desktop.getWindowTree(pid);
        console.log(`Total elements with default config: ${countElements(defaultTree)}`);
        
    } catch (error) {
        console.error('Error:', error);
    }
}

function countElements(node) {
    let count = 1; // Count this node
    if (node.children) {
        for (const child of node.children) {
            count += countElements(child);
        }
    }
    return count;
}

// Run the test
testGetWindowTree(); 