const native = require('./index.js');

// Custom error classes
class ElementNotFoundError extends Error {
    constructor(message) {
        super(message);
        this.name = 'ElementNotFoundError';
    }
}

class TimeoutError extends Error {
    constructor(message) {
        super(message);
        this.name = 'TimeoutError';
    }
}

class PermissionDeniedError extends Error {
    constructor(message) {
        super(message);
        this.name = 'PermissionDeniedError';
    }
}

class PlatformError extends Error {
    constructor(message) {
        super(message);
        this.name = 'PlatformError';
    }
}

class UnsupportedOperationError extends Error {
    constructor(message) {
        super(message);
        this.name = 'UnsupportedOperationError';
    }
}

class UnsupportedPlatformError extends Error {
    constructor(message) {
        super(message);
        this.name = 'UnsupportedPlatformError';
    }
}

class InvalidArgumentError extends Error {
    constructor(message) {
        super(message);
        this.name = 'InvalidArgumentError';
    }
}

class InternalError extends Error {
    constructor(message) {
        super(message);
        this.name = 'InternalError';
    }
}

// Error mapping function
function mapNativeError(error) {
    if (!error.message) return error;
    
    const message = error.message;
    if (message.startsWith('ELEMENT_NOT_FOUND:')) {
        return new ElementNotFoundError(message.replace('ELEMENT_NOT_FOUND:', '').trim());
    }
    if (message.startsWith('OPERATION_TIMED_OUT:')) {
        return new TimeoutError(message.replace('OPERATION_TIMED_OUT:', '').trim());
    }
    if (message.startsWith('PERMISSION_DENIED:')) {
        return new PermissionDeniedError(message.replace('PERMISSION_DENIED:', '').trim());
    }
    if (message.startsWith('PLATFORM_ERROR:')) {
        return new PlatformError(message.replace('PLATFORM_ERROR:', '').trim());
    }
    if (message.startsWith('UNSUPPORTED_OPERATION:')) {
        return new UnsupportedOperationError(message.replace('UNSUPPORTED_OPERATION:', '').trim());
    }
    if (message.startsWith('UNSUPPORTED_PLATFORM:')) {
        return new UnsupportedPlatformError(message.replace('UNSUPPORTED_PLATFORM:', '').trim());
    }
    if (message.startsWith('INVALID_ARGUMENT:')) {
        return new InvalidArgumentError(message.replace('INVALID_ARGUMENT:', '').trim());
    }
    if (message.startsWith('INTERNAL_ERROR:')) {
        return new InternalError(message.replace('INTERNAL_ERROR:', '').trim());
    }
    return error;
}

// Wrap native functions to handle errors
function wrapNativeFunction(fn) {
    if (typeof fn !== 'function') return fn;
    return function(...args) {
        try {
            const result = fn.apply(this, args);
            if (result instanceof Promise) {
                return result.catch(error => {
                    throw mapNativeError(error);
                });
            }
            return result;
        } catch (error) {
            throw mapNativeError(error);
        }
    };
}

// Wrap all methods of a class
function wrapClassMethods(Class) {
    const prototype = Class.prototype;
    const methods = Object.getOwnPropertyNames(prototype);
    
    methods.forEach(method => {
        if (method !== 'constructor' && typeof prototype[method] === 'function') {
            prototype[method] = wrapNativeFunction(prototype[method]);
        }
    });
    
    return Class;
}

// Wrap the native classes
const Desktop = wrapClassMethods(native.Desktop);
const Element = wrapClassMethods(native.Element);
const Locator = wrapClassMethods(native.Locator);

// Export everything
module.exports = {
    Desktop,
    Element,
    Locator,
    // Export error classes
    ElementNotFoundError,
    TimeoutError,
    PermissionDeniedError,
    PlatformError,
    UnsupportedOperationError,
    UnsupportedPlatformError,
    InvalidArgumentError,
    InternalError
}; 