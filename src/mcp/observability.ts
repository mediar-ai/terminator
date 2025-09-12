/**
 * MCP-style Tool Observability System
 * 
 * This module provides comprehensive observability capabilities for MCP tool operations,
 * including execution tracking, result logging, debug information collection, and
 * workflow tracing across multiple tool invocations.
 * 
 * Features:
 * - Real-time tool execution tracking
 * - Comprehensive result logging with metadata
 * - Debug information collection and analysis
 * - Workflow tracing for multi-tool operations
 * - Performance monitoring and alerting
 * - Event-driven architecture with observers
 */

import { EventEmitter } from 'events';
import {
  ToolExecutionResult,
  WorkflowTrace,
  ToolExecutionObserver,
  ExecutionStatus,
  ToolRegistryEntry,
  ToolOperationType,
  AnnotationAudience,
  ToolPriority,
  PerformanceMetrics
} from './types.js';

/**
 * Central registry for tracking tool operations and workflows
 */
export class ToolObservabilityRegistry extends EventEmitter {
  private executionResults: Map<string, ToolExecutionResult> = new Map();
  private workflowTraces: Map<string, WorkflowTrace> = new Map();
  private registeredTools: Map<string, ToolRegistryEntry> = new Map();
  private observers: Set<ToolExecutionObserver> = new Set();
  private activeOperations: Map<string, ToolExecutionResult> = new Map();
  private performanceThresholds: Map<string, number> = new Map();

  constructor() {
    super();
    this.setupDefaultThresholds();
  }

  private setupDefaultThresholds(): void {
    this.performanceThresholds.set('default_duration', 5000); // 5 seconds
    this.performanceThresholds.set('default_memory', 50 * 1024 * 1024); // 50MB
    this.performanceThresholds.set('critical_duration', 30000); // 30 seconds
    this.performanceThresholds.set('critical_memory', 200 * 1024 * 1024); // 200MB
  }

  /**
   * Register a tool in the observability system
   */
  registerTool(entry: Omit<ToolRegistryEntry, 'registeredAt' | 'usageCount'>): void {
    const fullEntry: ToolRegistryEntry = {
      ...entry,
      registeredAt: new Date().toISOString(),
      usageCount: 0
    };

    this.registeredTools.set(entry.id, fullEntry);
    this.emit('toolRegistered', fullEntry);
  }

  /**
   * Add an observer for tool execution events
   */
  addObserver(observer: ToolExecutionObserver): void {
    this.observers.add(observer);
  }

  /**
   * Remove an observer
   */
  removeObserver(observer: ToolExecutionObserver): void {
    this.observers.delete(observer);
  }

  /**
   * Record a tool operation start
   */
  recordOperationStart(result: ToolExecutionResult): void {
    this.activeOperations.set(result.metadata.id, result);
    this.executionResults.set(result.metadata.id, result);
    
    // Update tool usage stats
    const toolEntry = this.registeredTools.get(result.metadata.toolName);
    if (toolEntry) {
      toolEntry.usageCount++;
      toolEntry.lastUsed = new Date().toISOString();
      this.registeredTools.set(toolEntry.id, toolEntry);
    }

    // Notify observers
    this.observers.forEach(observer => {
      observer.onOperationStart?.(result);
    });

    this.emit('operationStart', result);
  }

  /**
   * Record a tool operation completion (success or failure)
   */
  recordOperationComplete(result: ToolExecutionResult): void {
    this.activeOperations.delete(result.metadata.id);
    this.executionResults.set(result.metadata.id, result);

    // Check performance thresholds
    this.checkPerformanceThresholds(result);

    // Notify observers based on status
    this.observers.forEach(observer => {
      switch (result.status) {
        case ExecutionStatus.SUCCESS:
          observer.onOperationSuccess?.(result);
          break;
        case ExecutionStatus.ERROR:
        case ExecutionStatus.TIMEOUT:
          observer.onOperationError?.(result);
          break;
        case ExecutionStatus.CANCELLED:
          observer.onOperationCancelled?.(result);
          break;
      }
    });

    this.emit('operationComplete', result);
  }

  /**
   * Start tracking a workflow
   */
  startWorkflow(workflowId: string, name: string, metadata: Record<string, any> = {}): WorkflowTrace {
    const trace: WorkflowTrace = {
      workflowId,
      name,
      startTime: new Date().toISOString(),
      status: ExecutionStatus.RUNNING,
      operations: [],
      metadata
    };

    this.workflowTraces.set(workflowId, trace);

    // Notify observers
    this.observers.forEach(observer => {
      observer.onWorkflowStart?.(trace);
    });

    this.emit('workflowStart', trace);
    return trace;
  }

  /**
   * Complete a workflow
   */
  completeWorkflow(workflowId: string, status: ExecutionStatus = ExecutionStatus.SUCCESS): WorkflowTrace | null {
    const trace = this.workflowTraces.get(workflowId);
    if (!trace) return null;

    trace.endTime = new Date().toISOString();
    trace.status = status;

    // Collect all operations that belong to this workflow
    trace.operations = Array.from(this.executionResults.values())
      .filter(result => result.metadata.context?.parentId === workflowId);

    // Calculate workflow performance metrics
    if (trace.operations.length > 0) {
      trace.performance = this.aggregatePerformanceMetrics(trace.operations);
    }

    this.workflowTraces.set(workflowId, trace);

    // Notify observers
    this.observers.forEach(observer => {
      observer.onWorkflowComplete?.(trace);
    });

    this.emit('workflowComplete', trace);
    return trace;
  }

  /**
   * Get all execution results, optionally filtered
   */
  getExecutionResults(filter?: {
    toolName?: string;
    status?: ExecutionStatus;
    operationType?: ToolOperationType;
    since?: string;
    until?: string;
  }): ToolExecutionResult[] {
    let results = Array.from(this.executionResults.values());

    if (filter) {
      if (filter.toolName) {
        results = results.filter(r => r.metadata.toolName === filter.toolName);
      }
      if (filter.status) {
        results = results.filter(r => r.status === filter.status);
      }
      if (filter.operationType) {
        results = results.filter(r => r.metadata.operationType === filter.operationType);
      }
      if (filter.since) {
        results = results.filter(r => r.metadata.startTime >= filter.since!);
      }
      if (filter.until) {
        results = results.filter(r => r.metadata.startTime <= filter.until!);
      }
    }

    return results.sort((a, b) => 
      new Date(b.metadata.startTime).getTime() - new Date(a.metadata.startTime).getTime()
    );
  }

  /**
   * Get workflow traces, optionally filtered
   */
  getWorkflowTraces(filter?: {
    status?: ExecutionStatus;
    name?: string;
    since?: string;
    until?: string;
  }): WorkflowTrace[] {
    let traces = Array.from(this.workflowTraces.values());

    if (filter) {
      if (filter.status) {
        traces = traces.filter(t => t.status === filter.status);
      }
      if (filter.name) {
        traces = traces.filter(t => t.name.includes(filter.name!));
      }
      if (filter.since) {
        traces = traces.filter(t => t.startTime >= filter.since!);
      }
      if (filter.until) {
        traces = traces.filter(t => t.startTime <= filter.until!);
      }
    }

    return traces.sort((a, b) => 
      new Date(b.startTime).getTime() - new Date(a.startTime).getTime()
    );
  }

  /**
   * Get registered tools
   */
  getRegisteredTools(): ToolRegistryEntry[] {
    return Array.from(this.registeredTools.values())
      .sort((a, b) => b.usageCount - a.usageCount);
  }

  /**
   * Get currently active operations
   */
  getActiveOperations(): ToolExecutionResult[] {
    return Array.from(this.activeOperations.values());
  }

  /**
   * Get performance analytics
   */
  getPerformanceAnalytics(timeRange?: { since: string; until: string }): {
    totalOperations: number;
    successRate: number;
    averageDuration: number;
    slowestOperations: ToolExecutionResult[];
    errorOperations: ToolExecutionResult[];
    toolUsageStats: Array<{ toolName: string; count: number; averageDuration: number; successRate: number }>;
  } {
    const results = this.getExecutionResults(timeRange);
    
    const totalOperations = results.length;
    const successfulOperations = results.filter(r => r.status === ExecutionStatus.SUCCESS).length;
    const successRate = totalOperations > 0 ? (successfulOperations / totalOperations) * 100 : 0;
    
    const durations = results
      .filter(r => r.metadata.duration !== undefined)
      .map(r => r.metadata.duration!);
    const averageDuration = durations.length > 0 
      ? durations.reduce((sum, d) => sum + d, 0) / durations.length 
      : 0;
    
    const slowestOperations = results
      .filter(r => r.metadata.duration !== undefined)
      .sort((a, b) => (b.metadata.duration || 0) - (a.metadata.duration || 0))
      .slice(0, 10);
    
    const errorOperations = results.filter(r => r.status === ExecutionStatus.ERROR);
    
    // Tool usage statistics
    const toolStats = new Map<string, { count: number; durations: number[]; successes: number }>();
    
    results.forEach(result => {
      const toolName = result.metadata.toolName;
      if (!toolStats.has(toolName)) {
        toolStats.set(toolName, { count: 0, durations: [], successes: 0 });
      }
      
      const stats = toolStats.get(toolName)!;
      stats.count++;
      if (result.metadata.duration !== undefined) {
        stats.durations.push(result.metadata.duration);
      }
      if (result.status === ExecutionStatus.SUCCESS) {
        stats.successes++;
      }
    });
    
    const toolUsageStats = Array.from(toolStats.entries()).map(([toolName, stats]) => ({
      toolName,
      count: stats.count,
      averageDuration: stats.durations.length > 0 
        ? stats.durations.reduce((sum, d) => sum + d, 0) / stats.durations.length 
        : 0,
      successRate: stats.count > 0 ? (stats.successes / stats.count) * 100 : 0
    })).sort((a, b) => b.count - a.count);

    return {
      totalOperations,
      successRate,
      averageDuration,
      slowestOperations,
      errorOperations,
      toolUsageStats
    };
  }

  /**
   * Set performance thresholds for alerting
   */
  setPerformanceThreshold(metric: string, value: number): void {
    this.performanceThresholds.set(metric, value);
  }

  /**
   * Clear old execution results to prevent memory leaks
   */
  cleanup(olderThan: string): void {
    const cutoffTime = new Date(olderThan).getTime();
    
    for (const [id, result] of this.executionResults.entries()) {
      const resultTime = new Date(result.metadata.startTime).getTime();
      if (resultTime < cutoffTime) {
        this.executionResults.delete(id);
      }
    }
    
    for (const [id, trace] of this.workflowTraces.entries()) {
      const traceTime = new Date(trace.startTime).getTime();
      if (traceTime < cutoffTime) {
        this.workflowTraces.delete(id);
      }
    }
    
    this.emit('cleanup', { cutoffTime: olderThan });
  }

  private checkPerformanceThresholds(result: ToolExecutionResult): void {
    const duration = result.metadata.duration;
    const memory = result.performance?.memoryUsage;

    // Check duration thresholds
    if (duration) {
      const criticalThreshold = this.performanceThresholds.get('critical_duration');
      const defaultThreshold = this.performanceThresholds.get('default_duration');
      
      if (criticalThreshold && duration > criticalThreshold) {
        this.emit('performanceAlert', {
          type: 'critical_duration',
          result,
          threshold: criticalThreshold,
          actual: duration
        });
      } else if (defaultThreshold && duration > defaultThreshold) {
        this.emit('performanceAlert', {
          type: 'slow_operation',
          result,
          threshold: defaultThreshold,
          actual: duration
        });
      }
    }

    // Check memory thresholds
    if (memory) {
      const criticalThreshold = this.performanceThresholds.get('critical_memory');
      const defaultThreshold = this.performanceThresholds.get('default_memory');
      
      if (criticalThreshold && memory > criticalThreshold) {
        this.emit('performanceAlert', {
          type: 'critical_memory',
          result,
          threshold: criticalThreshold,
          actual: memory
        });
      } else if (defaultThreshold && memory > defaultThreshold) {
        this.emit('performanceAlert', {
          type: 'high_memory',
          result,
          threshold: defaultThreshold,
          actual: memory
        });
      }
    }
  }

  private aggregatePerformanceMetrics(operations: ToolExecutionResult[]): PerformanceMetrics {
    const metrics: PerformanceMetrics = {};
    
    const memoryUsages = operations
      .map(op => op.performance?.memoryUsage)
      .filter((usage): usage is number => usage !== undefined);
    
    if (memoryUsages.length > 0) {
      metrics.memoryUsage = memoryUsages.reduce((sum, usage) => sum + usage, 0);
    }
    
    const durations = operations
      .map(op => op.metadata.duration)
      .filter((duration): duration is number => duration !== undefined);
    
    if (durations.length > 0) {
      metrics.custom = {
        totalDuration: durations.reduce((sum, duration) => sum + duration, 0),
        averageDuration: durations.reduce((sum, duration) => sum + duration, 0) / durations.length,
        operationCount: operations.length
      };
    }
    
    return metrics;
  }
}

/**
 * Debug information collector
 */
export class DebugCollector implements ToolExecutionObserver {
  private debugLogs: Array<{
    timestamp: string;
    level: 'info' | 'warn' | 'error' | 'debug';
    message: string;
    metadata?: any;
  }> = [];

  private maxLogs = 1000;

  onOperationStart(result: ToolExecutionResult): void {
    this.addLog('info', `Operation started: ${result.metadata.toolName}`, {
      operationId: result.metadata.id,
      operationType: result.metadata.operationType
    });
  }

  onOperationSuccess(result: ToolExecutionResult): void {
    this.addLog('info', `Operation completed: ${result.metadata.toolName}`, {
      operationId: result.metadata.id,
      duration: result.metadata.duration
    });
  }

  onOperationError(result: ToolExecutionResult): void {
    this.addLog('error', `Operation failed: ${result.metadata.toolName}`, {
      operationId: result.metadata.id,
      error: result.error,
      duration: result.metadata.duration
    });
  }

  onWorkflowStart(trace: WorkflowTrace): void {
    this.addLog('info', `Workflow started: ${trace.name}`, {
      workflowId: trace.workflowId
    });
  }

  onWorkflowComplete(trace: WorkflowTrace): void {
    this.addLog('info', `Workflow completed: ${trace.name}`, {
      workflowId: trace.workflowId,
      status: trace.status,
      operationCount: trace.operations.length
    });
  }

  addLog(level: 'info' | 'warn' | 'error' | 'debug', message: string, metadata?: any): void {
    this.debugLogs.push({
      timestamp: new Date().toISOString(),
      level,
      message,
      metadata
    });

    // Maintain max log size
    if (this.debugLogs.length > this.maxLogs) {
      this.debugLogs.shift();
    }
  }

  getLogs(filter?: {
    level?: 'info' | 'warn' | 'error' | 'debug';
    since?: string;
    until?: string;
    search?: string;
  }): typeof this.debugLogs {
    let logs = [...this.debugLogs];

    if (filter) {
      if (filter.level) {
        logs = logs.filter(log => log.level === filter.level);
      }
      if (filter.since) {
        logs = logs.filter(log => log.timestamp >= filter.since!);
      }
      if (filter.until) {
        logs = logs.filter(log => log.timestamp <= filter.until!);
      }
      if (filter.search) {
        const searchTerm = filter.search.toLowerCase();
        logs = logs.filter(log => 
          log.message.toLowerCase().includes(searchTerm) ||
          JSON.stringify(log.metadata).toLowerCase().includes(searchTerm)
        );
      }
    }

    return logs;
  }

  exportLogs(): string {
    return JSON.stringify(this.debugLogs, null, 2);
  }

  clearLogs(): void {
    this.debugLogs = [];
  }
}

/**
 * Global observability registry instance
 */
export const globalRegistry = new ToolObservabilityRegistry();

/**
 * Global debug collector instance
 */
export const globalDebugCollector = new DebugCollector();

// Attach debug collector to global registry by default
globalRegistry.addObserver(globalDebugCollector);

/**
 * Utility function to create a simple performance observer
 */
export function createPerformanceObserver(
  slowThreshold: number = 5000,
  memoryThreshold: number = 50 * 1024 * 1024
): ToolExecutionObserver {
  return {
    onOperationSuccess(result) {
      const duration = result.metadata.duration;
      const memory = result.performance?.memoryUsage;

      if (duration && duration > slowThreshold) {
        console.warn(`Slow operation detected: ${result.metadata.toolName} took ${duration}ms`);
      }

      if (memory && memory > memoryThreshold) {
        console.warn(`High memory usage detected: ${result.metadata.toolName} used ${Math.round(memory / 1024 / 1024)}MB`);
      }
    },

    onOperationError(result) {
      console.error(`Operation failed: ${result.metadata.toolName}`, result.error);
    }
  };
}

/**
 * Utility function to create a workflow logger
 */
export function createWorkflowLogger(): ToolExecutionObserver {
  return {
    onWorkflowStart(trace) {
      console.log(`🔄 Workflow started: ${trace.name} (${trace.workflowId})`);
    },

    onWorkflowComplete(trace) {
      const duration = trace.endTime 
        ? new Date(trace.endTime).getTime() - new Date(trace.startTime).getTime()
        : 0;

      console.log(`✅ Workflow completed: ${trace.name} (${trace.workflowId}) - ${trace.operations.length} operations in ${duration}ms`);
    },

    onOperationStart(result) {
      console.log(`  🔧 ${result.metadata.toolName}: ${result.metadata.description}`);
    }
  };
}

/**
 * Export convenience functions
 */
export {
  ExecutionStatus,
  ToolOperationType,
  AnnotationAudience,
  ToolPriority
} from './types.js';