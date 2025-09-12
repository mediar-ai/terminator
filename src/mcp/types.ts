/**
 * MCP-style Tool Annotations Types
 * 
 * This module provides TypeScript interfaces for MCP (Model Context Protocol)
 * tool annotations, enabling debugging and observability of tool workflows.
 * 
 * Based on MCP specification patterns for tool metadata and result tracking.
 */

/**
 * Supported tool operation types
 */
export enum ToolOperationType {
  READ = 'read',
  WRITE = 'write',
  EXECUTE = 'execute',
  SEARCH = 'search',
  TRANSFORM = 'transform',
  VALIDATE = 'validate',
  QUERY = 'query',
  UPDATE = 'update',
  DELETE = 'delete',
  CREATE = 'create'
}

/**
 * Target audience for tool annotations
 */
export enum AnnotationAudience {
  DEVELOPER = 'developer',
  USER = 'user',
  SYSTEM = 'system',
  DEBUG = 'debug'
}

/**
 * Priority levels for tool operations
 */
export enum ToolPriority {
  LOW = 'low',
  MEDIUM = 'medium',
  HIGH = 'high',
  CRITICAL = 'critical'
}

/**
 * Tool execution status
 */
export enum ExecutionStatus {
  PENDING = 'pending',
  RUNNING = 'running',
  SUCCESS = 'success',
  ERROR = 'error',
  CANCELLED = 'cancelled',
  TIMEOUT = 'timeout'
}

/**
 * Core tool annotation metadata following MCP patterns
 */
export interface ToolAnnotationMetadata {
  /** Unique identifier for this tool operation */
  id: string;
  
  /** Type of tool operation being performed */
  operationType: ToolOperationType;
  
  /** Target audience for this annotation */
  audience: AnnotationAudience;
  
  /** Priority level of this operation */
  priority: ToolPriority;
  
  /** ISO timestamp when operation started */
  startTime: string;
  
  /** ISO timestamp when operation completed (if applicable) */
  endTime?: string;
  
  /** Duration in milliseconds */
  duration?: number;
  
  /** Tool name or identifier */
  toolName: string;
  
  /** Human-readable description of the operation */
  description: string;
  
  /** Optional tags for categorization */
  tags?: string[];
  
  /** Context information about where this operation occurs */
  context?: {
    /** File path or resource identifier */
    resource?: string;
    /** Function or method name */
    function?: string;
    /** Line number or location */
    location?: string;
    /** Parent operation ID for nested operations */
    parentId?: string;
  };
}

/**
 * Input parameters for a tool operation
 */
export interface ToolInput {
  /** Primary input data */
  data: any;
  
  /** Optional configuration parameters */
  options?: Record<string, any>;
  
  /** Input validation schema reference */
  schema?: string;
  
  /** Input size in bytes (if applicable) */
  size?: number;
  
  /** Input type/format */
  type?: string;
}

/**
 * Output result from a tool operation
 */
export interface ToolOutput {
  /** Primary output data */
  data: any;
  
  /** Output metadata */
  metadata?: Record<string, any>;
  
  /** Output size in bytes (if applicable) */
  size?: number;
  
  /** Output type/format */
  type?: string;
  
  /** Validation status */
  isValid?: boolean;
  
  /** Validation errors if any */
  validationErrors?: string[];
}

/**
 * Error information for failed operations
 */
export interface ToolError {
  /** Error code or type */
  code: string;
  
  /** Human-readable error message */
  message: string;
  
  /** Detailed error description */
  details?: string;
  
  /** Stack trace (if available) */
  stack?: string;
  
  /** Error severity */
  severity: 'warning' | 'error' | 'fatal';
  
  /** Suggested remediation steps */
  remediation?: string[];
  
  /** Related error context */
  context?: Record<string, any>;
}

/**
 * Performance metrics for tool operations
 */
export interface PerformanceMetrics {
  /** CPU usage percentage */
  cpuUsage?: number;
  
  /** Memory usage in bytes */
  memoryUsage?: number;
  
  /** Network I/O bytes */
  networkIO?: number;
  
  /** Disk I/O bytes */
  diskIO?: number;
  
  /** Cache hit/miss ratio */
  cacheRatio?: number;
  
  /** Custom metrics */
  custom?: Record<string, number>;
}

/**
 * Complete tool execution result with full observability data
 */
export interface ToolExecutionResult {
  /** Core annotation metadata */
  metadata: ToolAnnotationMetadata;
  
  /** Tool input parameters */
  input: ToolInput;
  
  /** Tool output result (if successful) */
  output?: ToolOutput;
  
  /** Error information (if failed) */
  error?: ToolError;
  
  /** Current execution status */
  status: ExecutionStatus;
  
  /** Performance metrics */
  performance?: PerformanceMetrics;
  
  /** Child operations spawned by this tool */
  children?: ToolExecutionResult[];
  
  /** Execution environment information */
  environment?: {
    /** Operating system */
    os?: string;
    /** Runtime version */
    runtime?: string;
    /** Process ID */
    pid?: number;
    /** Working directory */
    workingDirectory?: string;
  };
  
  /** Debug information */
  debug?: {
    /** Intermediate states */
    states?: any[];
    /** Debug logs */
    logs?: string[];
    /** Breakpoint information */
    breakpoints?: string[];
  };
}

/**
 * Configuration for tool annotation system
 */
export interface ToolAnnotationConfig {
  /** Enable/disable annotation system */
  enabled: boolean;
  
  /** Default annotation audience */
  defaultAudience: AnnotationAudience;
  
  /** Default priority level */
  defaultPriority: ToolPriority;
  
  /** Enable performance monitoring */
  enablePerformanceTracking: boolean;
  
  /** Enable debug information collection */
  enableDebugInfo: boolean;
  
  /** Maximum number of child operations to track */
  maxChildOperations?: number;
  
  /** Timeout for operations in milliseconds */
  operationTimeout?: number;
  
  /** Custom metadata extractors */
  metadataExtractors?: Record<string, (input: any) => Record<string, any>>;
  
  /** Result serialization options */
  serialization?: {
    /** Include input data in results */
    includeInput?: boolean;
    /** Include output data in results */
    includeOutput?: boolean;
    /** Maximum serialized data size */
    maxDataSize?: number;
  };
}

/**
 * Tool registry entry for managing annotated tools
 */
export interface ToolRegistryEntry {
  /** Tool identifier */
  id: string;
  
  /** Tool name */
  name: string;
  
  /** Tool description */
  description: string;
  
  /** Tool version */
  version: string;
  
  /** Supported operation types */
  supportedOperations: ToolOperationType[];
  
  /** Default configuration */
  defaultConfig: Partial<ToolAnnotationConfig>;
  
  /** Tool-specific metadata schema */
  metadataSchema?: Record<string, any>;
  
  /** Registration timestamp */
  registeredAt: string;
  
  /** Last used timestamp */
  lastUsed?: string;
  
  /** Usage count */
  usageCount: number;
}

/**
 * Workflow trace information for multi-tool operations
 */
export interface WorkflowTrace {
  /** Unique workflow identifier */
  workflowId: string;
  
  /** Workflow name or description */
  name: string;
  
  /** Workflow start time */
  startTime: string;
  
  /** Workflow end time (if completed) */
  endTime?: string;
  
  /** Overall workflow status */
  status: ExecutionStatus;
  
  /** All tool operations in this workflow */
  operations: ToolExecutionResult[];
  
  /** Workflow-level metadata */
  metadata: Record<string, any>;
  
  /** Workflow performance metrics */
  performance?: PerformanceMetrics;
  
  /** Workflow context */
  context?: {
    /** User or system that initiated the workflow */
    initiator?: string;
    /** Workflow trigger */
    trigger?: string;
    /** Related workflows */
    relatedWorkflows?: string[];
  };
}

/**
 * Observer interface for tool execution events
 */
export interface ToolExecutionObserver {
  /** Called when a tool operation starts */
  onOperationStart?(result: ToolExecutionResult): void;
  
  /** Called when a tool operation completes successfully */
  onOperationSuccess?(result: ToolExecutionResult): void;
  
  /** Called when a tool operation fails */
  onOperationError?(result: ToolExecutionResult): void;
  
  /** Called when a tool operation is cancelled */
  onOperationCancelled?(result: ToolExecutionResult): void;
  
  /** Called when a workflow starts */
  onWorkflowStart?(trace: WorkflowTrace): void;
  
  /** Called when a workflow completes */
  onWorkflowComplete?(trace: WorkflowTrace): void;
}

/**
 * Export all types as a namespace for convenience
 */
export namespace MCPToolAnnotations {
  export type Metadata = ToolAnnotationMetadata;
  export type Input = ToolInput;
  export type Output = ToolOutput;
  export type Error = ToolError;
  export type Result = ToolExecutionResult;
  export type Config = ToolAnnotationConfig;
  export type RegistryEntry = ToolRegistryEntry;
  export type Trace = WorkflowTrace;
  export type Observer = ToolExecutionObserver;
  export type Performance = PerformanceMetrics;
  
  export const OperationType = ToolOperationType;
  export const Audience = AnnotationAudience;
  export const Priority = ToolPriority;
  export const Status = ExecutionStatus;
}