#!/usr/bin/env node
/**
 * MCP Tool Annotations Demo
 * 
 * This demo showcases the MCP-style tool annotation system for debugging
 * and observability workflows. It demonstrates various annotation patterns,
 * performance monitoring, error handling, and workflow tracing.
 * 
 * Features demonstrated:
 * - Method decorators for automatic annotation
 * - Function wrappers for legacy code
 * - Async operation support with error handling
 * - Performance monitoring and metrics collection
 * - Workflow tracing across multiple operations
 * - Debug information collection
 * - Real-time observability with event listeners
 * 
 * Requirements: 
 * - TypeScript or tsx to run directly
 * 
 * Usage: tsx examples/mcp-annotations-demo.ts
 */

import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';
import { randomUUID } from 'crypto';

// Import our MCP annotation system
import {
  toolAnnotation,
  annotatedTool,
  wrapTool,
  AnnotatedFileOps,
  ToolOperationType,
  AnnotationAudience,
  ToolPriority,
  ExecutionStatus,
  setGlobalConfig,
  createTestAnnotation
} from '../src/mcp/tool-annotations.js';

import {
  globalRegistry,
  globalDebugCollector,
  createPerformanceObserver,
  createWorkflowLogger,
  DebugCollector
} from '../src/mcp/observability.js';

import {
  ToolExecutionResult,
  WorkflowTrace
} from '../src/mcp/types.js';

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Demo class showing method decorators
 */
class DemoCalculator {
  @toolAnnotation({
    operationType: ToolOperationType.EXECUTE,
    toolName: 'calculator.add',
    description: 'Add two numbers',
    audience: AnnotationAudience.USER,
    priority: ToolPriority.LOW,
    tags: ['math', 'basic', 'calculator']
  })
  async add(a: number, b: number): Promise<number> {
    // Simulate some processing time
    await sleep(100 + Math.random() * 200);
    return a + b;
  }

  @toolAnnotation({
    operationType: ToolOperationType.EXECUTE,
    toolName: 'calculator.divide',
    description: 'Divide two numbers',
    audience: AnnotationAudience.DEVELOPER,
    priority: ToolPriority.MEDIUM,
    tags: ['math', 'division', 'calculator']
  })
  async divide(a: number, b: number): Promise<number> {
    await sleep(150 + Math.random() * 300);
    
    if (b === 0) {
      throw new Error('Division by zero is not allowed');
    }
    
    return a / b;
  }

  @toolAnnotation({
    operationType: ToolOperationType.EXECUTE,
    toolName: 'calculator.complexOperation',
    description: 'Perform complex mathematical operation',
    audience: AnnotationAudience.SYSTEM,
    priority: ToolPriority.HIGH,
    tags: ['math', 'complex', 'heavy']
  })
  async complexOperation(x: number): Promise<number> {
    // Simulate a more expensive operation
    await sleep(500 + Math.random() * 1000);
    
    // Simulate memory usage
    const largeArray = new Array(100000).fill(x);
    const result = largeArray.reduce((sum, val) => sum + Math.sqrt(val), 0);
    
    return result / largeArray.length;
  }
}

/**
 * Legacy function to be wrapped
 */
function legacyDataProcessor(data: any[]): any {
  // Simulate processing
  return data.map(item => ({
    ...item,
    processed: true,
    timestamp: new Date().toISOString()
  }));
}

/**
 * Demo async function
 */
async function fetchUserData(userId: string): Promise<any> {
  await sleep(200 + Math.random() * 500);
  
  if (userId === 'invalid') {
    throw new Error('User not found');
  }
  
  return {
    id: userId,
    name: `User ${userId}`,
    email: `user${userId}@example.com`,
    createdAt: new Date().toISOString()
  };
}

async function main(): Promise<void> {
  console.log('🎯 MCP Tool Annotations Demo');
  console.log('='.repeat(50));

  // Configure the annotation system
  setGlobalConfig({
    enabled: true,
    defaultAudience: AnnotationAudience.DEVELOPER,
    defaultPriority: ToolPriority.MEDIUM,
    enablePerformanceTracking: true,
    enableDebugInfo: true,
    operationTimeout: 10000, // 10 seconds
    serialization: {
      includeInput: true,
      includeOutput: true,
      maxDataSize: 512 * 1024 // 512KB
    }
  });

  // Set up observers
  const perfObserver = createPerformanceObserver(300, 10 * 1024 * 1024); // 300ms, 10MB
  const workflowLogger = createWorkflowLogger();
  
  globalRegistry.addObserver(perfObserver);
  globalRegistry.addObserver(workflowLogger);
  
  // Register some tools
  globalRegistry.registerTool({
    id: 'demo-calculator',
    name: 'Demo Calculator',
    description: 'Simple calculator for demonstration',
    version: '1.0.0',
    supportedOperations: [ToolOperationType.EXECUTE],
    defaultConfig: {
      enabled: true,
      defaultPriority: ToolPriority.LOW
    }
  });

  globalRegistry.registerTool({
    id: 'data-processor',
    name: 'Data Processor',
    description: 'Legacy data processing functions',
    version: '1.0.0',
    supportedOperations: [ToolOperationType.TRANSFORM],
    defaultConfig: {
      enabled: true,
      defaultPriority: ToolPriority.MEDIUM
    }
  });

  try {
    // Demo 1: Method decorators with calculator
    console.log('\n📊 Demo 1: Method Decorators (Calculator)');
    console.log('-'.repeat(30));
    
    const calculator = new DemoCalculator();
    
    // These will be automatically annotated
    const sum = await calculator.add(10, 5);
    console.log(`10 + 5 = ${sum}`);
    
    const product = await calculator.divide(20, 4);
    console.log(`20 / 4 = ${product}`);
    
    // This will trigger a performance warning
    const complexResult = await calculator.complexOperation(16);
    console.log(`Complex operation result: ${complexResult.toFixed(2)}`);
    
    // Demo 2: Error handling
    console.log('\n❌ Demo 2: Error Handling');
    console.log('-'.repeat(30));
    
    try {
      await calculator.divide(10, 0);
    } catch (error) {
      console.log(`Caught expected error: ${error.message}`);
    }

    // Demo 3: Function wrappers
    console.log('\n🔄 Demo 3: Function Wrappers');
    console.log('-'.repeat(30));
    
    const wrappedProcessor = wrapTool(
      legacyDataProcessor,
      'legacyDataProcessor',
      ToolOperationType.TRANSFORM,
      'Process legacy data format'
    );
    
    const testData = [
      { id: 1, name: 'Item 1' },
      { id: 2, name: 'Item 2' },
      { id: 3, name: 'Item 3' }
    ];
    
    const processedData = await wrappedProcessor(testData);
    console.log(`Processed ${processedData.length} items`);
    
    // Demo 4: Manual annotation with annotatedTool
    console.log('\n🛠️ Demo 4: Manual Annotation');
    console.log('-'.repeat(30));
    
    const { result: userData, annotation } = await annotatedTool(
      () => fetchUserData('user123'),
      {
        operationType: ToolOperationType.QUERY,
        toolName: 'userFetcher',
        description: 'Fetch user data from API',
        audience: AnnotationAudience.SYSTEM,
        priority: ToolPriority.HIGH,
        tags: ['api', 'user', 'fetch'],
        context: {
          resource: 'users/user123',
          function: 'fetchUserData'
        },
        inputTransformer: (args) => ({ userId: args[0] }),
        outputTransformer: (result) => ({ 
          userId: result.id, 
          hasData: true,
          fieldCount: Object.keys(result).length
        }),
        onResult: (result) => {
          console.log(`  📝 Operation ${result.metadata.id} completed in ${result.metadata.duration}ms`);
        }
      }
    );
    
    console.log(`Fetched user: ${userData.name}`);
    console.log(`Annotation ID: ${annotation.metadata.id}`);

    // Demo 5: Workflow tracing
    console.log('\n🌊 Demo 5: Workflow Tracing');
    console.log('-'.repeat(30));
    
    const workflowId = randomUUID();
    const workflow = globalRegistry.startWorkflow(
      workflowId,
      'User Data Processing Workflow',
      { version: '1.0', description: 'Complete user data processing pipeline' }
    );
    
    // Perform multiple operations as part of the workflow
    const users = ['alice', 'bob', 'charlie'];
    const results = [];
    
    for (const userId of users) {
      try {
        const { result: user } = await annotatedTool(
          () => fetchUserData(userId),
          {
            operationType: ToolOperationType.QUERY,
            toolName: 'userFetcher',
            description: `Fetch user data for ${userId}`,
            context: { parentId: workflowId, resource: `users/${userId}` }
          }
        );
        
        const { result: processed } = await annotatedTool(
          () => wrappedProcessor([user]),
          {
            operationType: ToolOperationType.TRANSFORM,
            toolName: 'dataProcessor',
            description: `Process user data for ${userId}`,
            context: { parentId: workflowId, resource: `processing/${userId}` }
          }
        );
        
        results.push(processed[0]);
        console.log(`  ✅ Processed user: ${userId}`);
        
      } catch (error) {
        console.log(`  ❌ Failed to process user: ${userId}`);
      }
    }
    
    globalRegistry.completeWorkflow(workflowId, ExecutionStatus.SUCCESS);
    console.log(`Workflow completed with ${results.length} successful operations`);

    // Demo 6: File operations (if file system is available)
    if (typeof require !== 'undefined') {
      console.log('\n📁 Demo 6: File Operations');
      console.log('-'.repeat(30));
      
      try {
        const tempFile = `/tmp/mcp-demo-${Date.now()}.txt`;
        const content = `MCP Demo file created at ${new Date().toISOString()}`;
        
        await AnnotatedFileOps.writeFile(tempFile, content);
        console.log(`  📝 Created file: ${tempFile}`);
        
        const readContent = await AnnotatedFileOps.readFile(tempFile);
        console.log(`  📖 Read content: ${readContent.substring(0, 50)}...`);
        
        const stats = await AnnotatedFileOps.getStats(tempFile);
        console.log(`  📊 File size: ${stats.size} bytes`);
        
        await AnnotatedFileOps.deleteFile(tempFile);
        console.log(`  🗑️ Deleted file: ${tempFile}`);
        
      } catch (error) {
        console.log(`  ⚠️ File operations demo skipped: ${error.message}`);
      }
    }

    // Demo 7: Performance analytics
    console.log('\n📈 Demo 7: Performance Analytics');
    console.log('-'.repeat(30));
    
    await sleep(1000); // Wait for all operations to complete
    
    const analytics = globalRegistry.getPerformanceAnalytics();
    console.log(`Total operations: ${analytics.totalOperations}`);
    console.log(`Success rate: ${analytics.successRate.toFixed(1)}%`);
    console.log(`Average duration: ${analytics.averageDuration.toFixed(1)}ms`);
    console.log(`Error operations: ${analytics.errorOperations.length}`);
    
    console.log('\nTop 5 tools by usage:');
    analytics.toolUsageStats.slice(0, 5).forEach((tool, index) => {
      console.log(`  ${index + 1}. ${tool.toolName}: ${tool.count} ops, ${tool.averageDuration.toFixed(1)}ms avg, ${tool.successRate.toFixed(1)}% success`);
    });
    
    if (analytics.slowestOperations.length > 0) {
      console.log('\nSlowest operations:');
      analytics.slowestOperations.slice(0, 3).forEach((op, index) => {
        console.log(`  ${index + 1}. ${op.metadata.toolName}: ${op.metadata.duration?.toFixed(1)}ms`);
      });
    }

    // Demo 8: Debug information
    console.log('\n🐛 Demo 8: Debug Information');
    console.log('-'.repeat(30));
    
    const recentLogs = globalDebugCollector.getLogs({
      since: new Date(Date.now() - 60000).toISOString() // Last minute
    });
    
    console.log(`Recent debug logs: ${recentLogs.length} entries`);
    
    const logCounts = recentLogs.reduce((counts, log) => {
      counts[log.level] = (counts[log.level] || 0) + 1;
      return counts;
    }, {} as Record<string, number>);
    
    Object.entries(logCounts).forEach(([level, count]) => {
      console.log(`  ${level}: ${count} entries`);
    });
    
    // Show some recent logs
    console.log('\nRecent log entries:');
    recentLogs.slice(-5).forEach(log => {
      console.log(`  [${log.level.toUpperCase()}] ${log.message}`);
    });

    console.log('\n🎉 Demo completed successfully!');
    console.log('\nKey features demonstrated:');
    console.log('  ✅ Method decorators for automatic annotation');
    console.log('  ✅ Function wrappers for legacy code integration');
    console.log('  ✅ Manual annotation with full control');
    console.log('  ✅ Error handling and timeout management');
    console.log('  ✅ Workflow tracing across multiple operations');
    console.log('  ✅ Performance monitoring and analytics');
    console.log('  ✅ Debug information collection');
    console.log('  ✅ Real-time observability with event listeners');
    console.log('  ✅ Tool registry and usage statistics');

  } catch (error) {
    console.error('❌ Error during demo:', error);
    if (error instanceof Error) {
      console.error(error.stack);
    }
  } finally {
    // Cleanup
    console.log('\n🧹 Cleaning up...');
    
    // Show final statistics
    const finalStats = globalRegistry.getPerformanceAnalytics();
    console.log(`Final statistics: ${finalStats.totalOperations} operations, ${finalStats.successRate.toFixed(1)}% success rate`);
    
    // Export debug logs for analysis
    console.log('Debug logs exported to memory (would normally save to file)');
    
    // Show registered tools
    const tools = globalRegistry.getRegisteredTools();
    console.log(`Registered tools: ${tools.length}`);
    tools.forEach(tool => {
      console.log(`  - ${tool.name}: ${tool.usageCount} uses`);
    });
  }
}

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
});

// Performance monitoring for the demo itself
globalRegistry.on('performanceAlert', (alert) => {
  console.log(`⚡ Performance Alert: ${alert.type} - ${alert.actual} exceeds threshold ${alert.threshold}`);
});

globalRegistry.on('operationComplete', (result) => {
  // Only log errors and slow operations to avoid spam
  if (result.status === ExecutionStatus.ERROR) {
    console.log(`❌ Operation failed: ${result.metadata.toolName} - ${result.error?.message}`);
  } else if (result.metadata.duration && result.metadata.duration > 1000) {
    console.log(`⏰ Slow operation: ${result.metadata.toolName} took ${result.metadata.duration}ms`);
  }
});

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});