// Re-export all types and interfaces from the original declaration file
export * from './index.d';

/** Thrown when an element is not found. */
export class ElementNotFoundError extends Error {
    constructor(message: string);
}

/** Thrown when an operation times out. */
export class TimeoutError extends Error {
    constructor(message: string);
}

/** Thrown when permission is denied. */
export class PermissionDeniedError extends Error {
    constructor(message: string);
}

/** Thrown for platform-specific errors. */
export class PlatformError extends Error {
    constructor(message: string);
}

/** Thrown for unsupported operations. */
export class UnsupportedOperationError extends Error {
    constructor(message: string);
}

/** Thrown for unsupported platforms. */
export class UnsupportedPlatformError extends Error {
    constructor(message: string);
}

/** Thrown for invalid arguments. */
export class InvalidArgumentError extends Error {
    constructor(message: string);
}

/** Thrown for internal errors. */
export class InternalError extends Error {
    constructor(message: string);
} 