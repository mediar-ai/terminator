/**
 * MCP-style Tool Annotations - Decorator/Wrapper System
 * 
 * This module provides a decorator and wrapper system for annotating tool operations
 * with metadata, enabling comprehensive debugging and observability workflows.
 * 
 * Features:
 * - Method decorators for automatic annotation
 * - Function wrappers for legacy code integration
 * - Async operation support with proper error handling
 * - Performance monitoring and metrics collection
 * - Result caching and validation
 */

import { randomUUID } from 'crypto';
import {
  ToolAnnotationMetadata,
  ToolExecutionResult,
  ToolInput,
  ToolOutput,
  ToolError,
  ToolOperationType,
  AnnotationAudience,
  ToolPriority,
  ExecutionStatus,
  PerformanceMetrics,
  ToolAnnotationConfig
} from './types.js';

/**
 * Default configuration for tool annotations
 */
const DEFAULT_CONFIG: ToolAnnotationConfig = {
  enabled: true,
  defaultAudience: AnnotationAudience.DEVELOPER,
  defaultPriority: ToolPriority.MEDIUM,
  enablePerformanceTracking: true,
  enableDebugInfo: true,
  maxChildOperations: 50,
  operationTimeout: 30000, // 30 seconds
  serialization: {
    includeInput: true,
    includeOutput: true,
    maxDataSize: 1024 * 1024 // 1MB
  }
};

/**
 * Global configuration instance
 */
let globalConfig: ToolAnnotationConfig = { ...DEFAULT_CONFIG };

/**
 * Set global configuration for tool annotations
 */
export function setGlobalConfig(config: Partial<ToolAnnotationConfig>): void {
  globalConfig = { ...globalConfig, ...config };
}

/**
 * Get current global configuration
 */
export function getGlobalConfig(): ToolAnnotationConfig {
  return { ...globalConfig };
}

/**
 * Performance monitor utility
 */
class PerformanceMonitor {
  private startTime: number = 0;
  private startMemory: number = 0;

  start(): void {
    this.startTime = performance.now();
    if (typeof process !== 'undefined' && process.memoryUsage) {
      this.startMemory = process.memoryUsage().heapUsed;
    }
  }

  getMetrics(): PerformanceMetrics {
    const duration = performance.now() - this.startTime;
    const metrics: PerformanceMetrics = {};

    if (typeof process !== 'undefined' && process.memoryUsage) {
      const currentMemory = process.memoryUsage().heapUsed;
      metrics.memoryUsage = currentMemory - this.startMemory;
    }

    // Add custom duration metric
    metrics.custom = { duration };

    return metrics;
  }
}

/**
 * Result serializer with size limits
 */
class ResultSerializer {
  private maxSize: number;

  constructor(maxSize: number = 1024 * 1024) {
    this.maxSize = maxSize;
  }

  serialize(data: any): any {
    try {
      const serialized = JSON.stringify(data);
      if (serialized.length > this.maxSize) {
        return {
          _truncated: true,
          _originalSize: serialized.length,
          _preview: serialized.substring(0, 500) + '...',
          _summary: this.generateSummary(data)
        };
      }
      return JSON.parse(serialized);
    } catch (error) {
      return {
        _serializationError: true,
        _error: error instanceof Error ? error.message : 'Unknown error',
        _type: typeof data,
        _toString: String(data).substring(0, 200)
      };
    }
  }

  private generateSummary(data: any): any {
    if (data === null || data === undefined) return data;
    if (typeof data === 'string') return `"${data.substring(0, 50)}${data.length > 50 ? '...' : ''}"`;
    if (typeof data === 'number' || typeof data === 'boolean') return data;
    if (Array.isArray(data)) return `Array(${data.length})`;
    if (typeof data === 'object') return `Object(${Object.keys(data).length} keys)`;
    return typeof data;
  }
}

/**
 * Create metadata for a tool operation
 */
function createMetadata(options: {
  operationType: ToolOperationType;
  toolName: string;
  description: string;
  audience?: AnnotationAudience;
  priority?: ToolPriority;
  context?: ToolAnnotationMetadata['context'];
  tags?: string[];
}): ToolAnnotationMetadata {
  return {
    id: randomUUID(),
    operationType: options.operationType,
    audience: options.audience || globalConfig.defaultAudience,
    priority: options.priority || globalConfig.defaultPriority,
    startTime: new Date().toISOString(),
    toolName: options.toolName,
    description: options.description,
    tags: options.tags,
    context: options.context
  };
}

/**
 * Create tool input wrapper
 */
function createToolInput(data: any, options?: Record<string, any>): ToolInput {
  return {
    data,
    options,
    type: typeof data,
    size: typeof data === 'string' ? data.length : undefined
  };
}

/**
 * Create tool output wrapper
 */
function createToolOutput(data: any, metadata?: Record<string, any>): ToolOutput {
  return {
    data,
    metadata,
    type: typeof data,
    size: typeof data === 'string' ? data.length : undefined,
    isValid: true
  };
}

/**
 * Create tool error wrapper
 */
function createToolError(error: Error | any, severity: 'warning' | 'error' | 'fatal' = 'error'): ToolError {
  const err = error instanceof Error ? error : new Error(String(error));
  return {
    code: err.name || 'UnknownError',
    message: err.message,
    details: err.toString(),
    stack: err.stack,
    severity,
    context: error?.context || {}
  };
}

/**
 * Main tool annotation wrapper function
 */
export async function annotatedTool<T extends any[], R>(
  operation: (...args: T) => Promise<R> | R,
  options: {
    operationType: ToolOperationType;
    toolName: string;
    description: string;
    audience?: AnnotationAudience;
    priority?: ToolPriority;
    context?: ToolAnnotationMetadata['context'];
    tags?: string[];
    inputTransformer?: (args: T) => any;
    outputTransformer?: (result: R) => any;
    onResult?: (result: ToolExecutionResult) => void;
  }
): Promise<{ result: R; annotation: ToolExecutionResult }> {
  
  if (!globalConfig.enabled) {
    const result = await operation();
    return { 
      result,
      annotation: {} as ToolExecutionResult // Return empty annotation when disabled
    };
  }

  const metadata = createMetadata(options);
  const monitor = new PerformanceMonitor();
  const serializer = new ResultSerializer(globalConfig.serialization?.maxDataSize);

  let executionResult: ToolExecutionResult = {
    metadata,
    input: createToolInput(
      options.inputTransformer ? options.inputTransformer(arguments[0] as T) : arguments[0],
      { originalArgs: globalConfig.serialization?.includeInput ? serializer.serialize(arguments[0]) : undefined }
    ),
    status: ExecutionStatus.PENDING,
    children: [],
    environment: {
      os: typeof process !== 'undefined' ? process.platform : 'browser',
      runtime: typeof process !== 'undefined' ? process.version : 'browser',
      pid: typeof process !== 'undefined' ? process.pid : undefined,
      workingDirectory: typeof process !== 'undefined' ? process.cwd() : undefined
    },
    debug: globalConfig.enableDebugInfo ? { states: [], logs: [], breakpoints: [] } : undefined
  };

  try {
    // Start monitoring
    monitor.start();
    executionResult.status = ExecutionStatus.RUNNING;
    
    if (executionResult.debug) {
      executionResult.debug.logs?.push(`Operation started: ${options.toolName}`);
      executionResult.debug.states?.push({ state: 'started', timestamp: new Date().toISOString() });
    }

    // Execute operation with timeout
    let result: R;
    if (globalConfig.operationTimeout && globalConfig.operationTimeout > 0) {
      result = await Promise.race([
        Promise.resolve(operation()),
        new Promise<never>((_, reject) => 
          setTimeout(() => reject(new Error('Operation timeout')), globalConfig.operationTimeout)
        )
      ]);
    } else {
      result = await Promise.resolve(operation());
    }

    // Success path
    executionResult.status = ExecutionStatus.SUCCESS;
    executionResult.metadata.endTime = new Date().toISOString();
    executionResult.metadata.duration = performance.now() - (executionResult.metadata.startTime ? new Date(executionResult.metadata.startTime).getTime() : 0);
    
    if (globalConfig.serialization?.includeOutput) {
      executionResult.output = createToolOutput(
        options.outputTransformer ? options.outputTransformer(result) : result,
        { serialized: serializer.serialize(result) }
      );
    } else {
      executionResult.output = createToolOutput(
        options.outputTransformer ? options.outputTransformer(result) : result
      );
    }

    if (globalConfig.enablePerformanceTracking) {
      executionResult.performance = monitor.getMetrics();
    }

    if (executionResult.debug) {
      executionResult.debug.logs?.push(`Operation completed successfully`);
      executionResult.debug.states?.push({ state: 'completed', timestamp: new Date().toISOString() });
    }

    if (options.onResult) {
      options.onResult(executionResult);
    }

    return { result, annotation: executionResult };

  } catch (error) {
    // Error path
    executionResult.status = error instanceof Error && error.message === 'Operation timeout' 
      ? ExecutionStatus.TIMEOUT 
      : ExecutionStatus.ERROR;
    
    executionResult.metadata.endTime = new Date().toISOString();
    executionResult.metadata.duration = performance.now() - (executionResult.metadata.startTime ? new Date(executionResult.metadata.startTime).getTime() : 0);
    
    executionResult.error = createToolError(error);

    if (globalConfig.enablePerformanceTracking) {
      executionResult.performance = monitor.getMetrics();
    }

    if (executionResult.debug) {
      executionResult.debug.logs?.push(`Operation failed: ${executionResult.error.message}`);
      executionResult.debug.states?.push({ state: 'failed', timestamp: new Date().toISOString(), error: executionResult.error });
    }

    if (options.onResult) {
      options.onResult(executionResult);
    }

    throw error;
  }
}

/**
 * Method decorator for automatic tool annotation
 * 
 * Usage:
 * @toolAnnotation({
 *   operationType: ToolOperationType.READ,
 *   toolName: 'fileReader',
 *   description: 'Read file contents'
 * })
 * async readFile(path: string): Promise<string> { ... }
 */
export function toolAnnotation(options: {
  operationType: ToolOperationType;
  toolName: string;
  description: string;
  audience?: AnnotationAudience;
  priority?: ToolPriority;
  tags?: string[];
  inputTransformer?: (args: any[]) => any;
  outputTransformer?: (result: any) => any;
}) {
  return function<T extends any[], R>(
    target: any,
    propertyKey: string,
    descriptor: TypedPropertyDescriptor<(...args: T) => Promise<R> | R>
  ): TypedPropertyDescriptor<(...args: T) => Promise<R> | R> | void {
    
    if (!descriptor?.value) return;

    const originalMethod = descriptor.value;
    
    descriptor.value = async function(...args: T): Promise<R> {
      const context = {
        function: `${target.constructor.name}.${propertyKey}`,
        location: `${target.constructor.name}:${propertyKey}`
      };

      const { result } = await annotatedTool(
        () => originalMethod.apply(this, args),
        { ...options, context }
      );

      return result;
    } as any;

    return descriptor;
  };
}

/**
 * Simple wrapper function for quick tool annotation
 */
export function wrapTool<T extends any[], R>(
  fn: (...args: T) => Promise<R> | R,
  toolName: string,
  operationType: ToolOperationType = ToolOperationType.EXECUTE,
  description?: string
): (...args: T) => Promise<R> {
  return async (...args: T): Promise<R> => {
    const { result } = await annotatedTool(
      () => fn(...args),
      {
        operationType,
        toolName,
        description: description || `Execute ${toolName}`,
        context: { function: fn.name || toolName }
      }
    );
    return result;
  };
}

/**
 * File operation wrappers with automatic annotation
 */
export class AnnotatedFileOps {
  @toolAnnotation({
    operationType: ToolOperationType.READ,
    toolName: 'fileReader',
    description: 'Read file contents',
    tags: ['file', 'io', 'read']
  })
  static async readFile(path: string): Promise<string> {
    const fs = await import('fs/promises');
    return fs.readFile(path, 'utf-8');
  }

  @toolAnnotation({
    operationType: ToolOperationType.WRITE,
    toolName: 'fileWriter',
    description: 'Write file contents',
    tags: ['file', 'io', 'write']
  })
  static async writeFile(path: string, content: string): Promise<void> {
    const fs = await import('fs/promises');
    return fs.writeFile(path, content, 'utf-8');
  }

  @toolAnnotation({
    operationType: ToolOperationType.DELETE,
    toolName: 'fileDeleter',
    description: 'Delete file',
    tags: ['file', 'io', 'delete']
  })
  static async deleteFile(path: string): Promise<void> {
    const fs = await import('fs/promises');
    return fs.unlink(path);
  }

  @toolAnnotation({
    operationType: ToolOperationType.QUERY,
    toolName: 'fileStats',
    description: 'Get file statistics',
    tags: ['file', 'io', 'stats']
  })
  static async getStats(path: string): Promise<any> {
    const fs = await import('fs/promises');
    return fs.stat(path);
  }
}

/**
 * Network operation wrappers
 */
export class AnnotatedNetworkOps {
  @toolAnnotation({
    operationType: ToolOperationType.QUERY,
    toolName: 'httpGet',
    description: 'HTTP GET request',
    tags: ['http', 'network', 'get'],
    inputTransformer: (args) => ({ url: args[0], options: args[1] }),
    outputTransformer: (result) => ({
      status: result.status,
      headers: result.headers,
      bodyLength: result.data?.length || 0
    })
  })
  static async httpGet(url: string, options?: any): Promise<any> {
    // This would use your preferred HTTP client
    throw new Error('HTTP client not implemented - replace with your preferred library');
  }

  @toolAnnotation({
    operationType: ToolOperationType.CREATE,
    toolName: 'httpPost',
    description: 'HTTP POST request',
    tags: ['http', 'network', 'post']
  })
  static async httpPost(url: string, data: any, options?: any): Promise<any> {
    throw new Error('HTTP client not implemented - replace with your preferred library');
  }
}

/**
 * Database operation wrappers
 */
export class AnnotatedDbOps {
  @toolAnnotation({
    operationType: ToolOperationType.QUERY,
    toolName: 'dbSelect',
    description: 'Database SELECT query',
    tags: ['database', 'query', 'select']
  })
  static async select(query: string, params?: any[]): Promise<any[]> {
    throw new Error('Database client not implemented - replace with your preferred library');
  }

  @toolAnnotation({
    operationType: ToolOperationType.CREATE,
    toolName: 'dbInsert',
    description: 'Database INSERT operation',
    tags: ['database', 'insert', 'create']
  })
  static async insert(table: string, data: Record<string, any>): Promise<any> {
    throw new Error('Database client not implemented - replace with your preferred library');
  }

  @toolAnnotation({
    operationType: ToolOperationType.UPDATE,
    toolName: 'dbUpdate',
    description: 'Database UPDATE operation',
    tags: ['database', 'update']
  })
  static async update(table: string, data: Record<string, any>, where: Record<string, any>): Promise<any> {
    throw new Error('Database client not implemented - replace with your preferred library');
  }
}

/**
 * Export convenience functions
 */
export {
  ToolOperationType,
  AnnotationAudience,
  ToolPriority,
  ExecutionStatus
} from './types.js';

/**
 * Create a quick annotation for testing
 */
export function createTestAnnotation(toolName: string, data: any = null): ToolExecutionResult {
  const metadata = createMetadata({
    operationType: ToolOperationType.EXECUTE,
    toolName,
    description: `Test operation for ${toolName}`
  });

  return {
    metadata,
    input: createToolInput(data),
    output: createToolOutput({ success: true, timestamp: new Date().toISOString() }),
    status: ExecutionStatus.SUCCESS
  };
}