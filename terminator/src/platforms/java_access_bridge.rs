//! Java Access Bridge (JAB) support for Windows
//! 
//! This module provides Rust bindings and integration for Oracle's Java Access Bridge,
//! which enables assistive technologies to access Java applications on Windows.
//! 
//! The Java Access Bridge exposes the Java Accessibility API through a Windows DLL
//! (WindowsAccessBridge.dll), allowing communication with Java applications.

use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::{Arc, Mutex, Once};
use libloading::{Library, Symbol};
use crate::{AutomationError, UIElement, UIElementAttributes};
use tracing::{debug, error, info, warn};

// Constants from AccessBridgePackages.h
const MAX_STRING_SIZE: usize = 1024;
const SHORT_STRING_SIZE: usize = 256;

// Type definitions to match Java Access Bridge C API
pub type AccessibleContext = i64; // Changed to i64 to support 64-bit systems
pub type JavaObject = i64;       // JOBJECT64 equivalent
pub type VmId = i32;

/// Java Access Bridge version information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct AccessBridgeVersionInfo {
    pub vm_version: [u16; SHORT_STRING_SIZE],
    pub bridge_java_class_version: [u16; SHORT_STRING_SIZE], 
    pub bridge_java_dll_version: [u16; SHORT_STRING_SIZE],
    pub bridge_win_dll_version: [u16; SHORT_STRING_SIZE],
}

/// Accessible context information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct AccessibleContextInfo {
    pub name: [u16; MAX_STRING_SIZE],
    pub description: [u16; MAX_STRING_SIZE],
    pub role: [u16; SHORT_STRING_SIZE],
    pub states: [u16; SHORT_STRING_SIZE],
    pub index_in_parent: i32,
    pub children_count: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub accessible_component: bool,
    pub accessible_action: bool,
    pub accessible_selection: bool,
    pub accessible_text: bool,
    pub accessible_interfaces: bool,
}

/// Accessible text information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct AccessibleTextInfo {
    pub char_count: i32,
    pub caret_index: i32,
    pub index_at_point: i32,
}

/// Accessible text selection information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct AccessibleTextSelectionInfo {
    pub selection_start_index: i32,
    pub selection_end_index: i32,
    pub selected_text: [u16; MAX_STRING_SIZE],
}

/// Function pointer types for Java Access Bridge API calls
type InitializeAccessBridgeFn = unsafe extern "C" fn() -> bool;
type ShutdownAccessBridgeFn = unsafe extern "C" fn() -> bool;
type IsJavaWindowFn = unsafe extern "C" fn(hwnd: *mut c_void) -> bool;
type GetAccessibleContextFromHWNDFn = unsafe extern "C" fn(
    hwnd: *mut c_void,
    vm_id: *mut VmId,
    ac: *mut AccessibleContext,
) -> bool;
type GetAccessibleContextInfoFn = unsafe extern "C" fn(
    vm_id: VmId,
    ac: AccessibleContext,
    info: *mut AccessibleContextInfo,
) -> bool;
type GetAccessibleChildFromContextFn = unsafe extern "C" fn(
    vm_id: VmId,
    ac: AccessibleContext,
    index: i32,
) -> AccessibleContext;
type GetAccessibleParentFromContextFn = unsafe extern "C" fn(
    vm_id: VmId,
    ac: AccessibleContext,
) -> AccessibleContext;
type ReleaseJavaObjectFn = unsafe extern "C" fn(vm_id: VmId, object: JavaObject);
type GetVersionInfoFn = unsafe extern "C" fn(
    vm_id: VmId,
    info: *mut AccessBridgeVersionInfo,
) -> bool;

/// Event handler function pointer types
type AccessBridgeFocusGainedFp = unsafe extern "C" fn(vm_id: VmId, event: JavaObject, source: JavaObject);
type AccessBridgeFocusLostFp = unsafe extern "C" fn(vm_id: VmId, event: JavaObject, source: JavaObject);

type SetFocusGainedFn = unsafe extern "C" fn(fp: Option<AccessBridgeFocusGainedFp>);
type SetFocusLostFn = unsafe extern "C" fn(fp: Option<AccessBridgeFocusLostFp>);

/// Java Access Bridge API wrapper
pub struct JavaAccessBridge {
    library: Library,
    
    // Core functions
    initialize_access_bridge: Symbol<'static, InitializeAccessBridgeFn>,
    shutdown_access_bridge: Symbol<'static, ShutdownAccessBridgeFn>,
    is_java_window: Symbol<'static, IsJavaWindowFn>,
    get_accessible_context_from_hwnd: Symbol<'static, GetAccessibleContextFromHWNDFn>,
    get_accessible_context_info: Symbol<'static, GetAccessibleContextInfoFn>,
    get_accessible_child_from_context: Symbol<'static, GetAccessibleChildFromContextFn>,
    get_accessible_parent_from_context: Symbol<'static, GetAccessibleParentFromContextFn>,
    release_java_object: Symbol<'static, ReleaseJavaObjectFn>,
    get_version_info: Symbol<'static, GetVersionInfoFn>,
    
    // Event handlers
    set_focus_gained: Symbol<'static, SetFocusGainedFn>,
    set_focus_lost: Symbol<'static, SetFocusLostFn>,
    
    initialized: bool,
}

unsafe impl Send for JavaAccessBridge {}
unsafe impl Sync for JavaAccessBridge {}

static INIT: Once = Once::new();
static mut JAB_INSTANCE: Option<Arc<Mutex<JavaAccessBridge>>> = None;

impl JavaAccessBridge {
    /// Get or create the global Java Access Bridge instance
    pub fn get_instance() -> Result<Arc<Mutex<JavaAccessBridge>>, AutomationError> {
        unsafe {
            INIT.call_once(|| {
                match Self::new() {
                    Ok(jab) => {
                        JAB_INSTANCE = Some(Arc::new(Mutex::new(jab)));
                        info!("✅ Java Access Bridge initialized successfully");
                    }
                    Err(e) => {
                        error!("❌ Failed to initialize Java Access Bridge: {}", e);
                    }
                }
            });
            
            JAB_INSTANCE
                .as_ref()
                .ok_or_else(|| AutomationError::PlatformError(
                    "Java Access Bridge not available".to_string()
                ))
                .map(|arc| Arc::clone(arc))
        }
    }
    
    /// Create a new Java Access Bridge instance by loading the DLL
    fn new() -> Result<Self, AutomationError> {
        info!("🔧 Loading Java Access Bridge DLL...");
        
        // Try to load WindowsAccessBridge.dll
        let library = Library::new("WindowsAccessBridge.dll")
            .or_else(|_| Library::new("WindowsAccessBridge-64.dll"))
            .or_else(|_| Library::new("WindowsAccessBridge-32.dll"))
            .map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load Java Access Bridge DLL: {}. Make sure Java Access Bridge is installed.",
                    e
                ))
            })?;
            
        info!("✅ Java Access Bridge DLL loaded successfully");
        
        // Load function symbols
        let initialize_access_bridge = unsafe {
            library.get(b"initializeAccessBridge").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load initializeAccessBridge function: {}",
                    e
                ))
            })?
        };
        
        let shutdown_access_bridge = unsafe {
            library.get(b"shutdownAccessBridge").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load shutdownAccessBridge function: {}",
                    e
                ))
            })?
        };
        
        let is_java_window = unsafe {
            library.get(b"IsJavaWindow").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load IsJavaWindow function: {}",
                    e
                ))
            })?
        };
        
        let get_accessible_context_from_hwnd = unsafe {
            library.get(b"GetAccessibleContextFromHWND").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load GetAccessibleContextFromHWND function: {}",
                    e
                ))
            })?
        };
        
        let get_accessible_context_info = unsafe {
            library.get(b"GetAccessibleContextInfo").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load GetAccessibleContextInfo function: {}",
                    e
                ))
            })?
        };
        
        let get_accessible_child_from_context = unsafe {
            library.get(b"GetAccessibleChildFromContext").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load GetAccessibleChildFromContext function: {}",
                    e
                ))
            })?
        };
        
        let get_accessible_parent_from_context = unsafe {
            library.get(b"GetAccessibleParentFromContext").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load GetAccessibleParentFromContext function: {}",
                    e
                ))
            })?
        };
        
        let release_java_object = unsafe {
            library.get(b"ReleaseJavaObject").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load ReleaseJavaObject function: {}",
                    e
                ))
            })?
        };
        
        let get_version_info = unsafe {
            library.get(b"GetVersionInfo").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load GetVersionInfo function: {}",
                    e
                ))
            })?
        };
        
        let set_focus_gained = unsafe {
            library.get(b"SetFocusGained").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load SetFocusGained function: {}",
                    e
                ))
            })?
        };
        
        let set_focus_lost = unsafe {
            library.get(b"SetFocusLost").map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to load SetFocusLost function: {}",
                    e
                ))
            })?
        };
        
        // Create instance but don't initialize yet
        let mut instance = Self {
            library,
            initialize_access_bridge,
            shutdown_access_bridge,
            is_java_window,
            get_accessible_context_from_hwnd,
            get_accessible_context_info,
            get_accessible_child_from_context,
            get_accessible_parent_from_context,
            release_java_object,
            get_version_info,
            set_focus_gained,
            set_focus_lost,
            initialized: false,
        };
        
        // Initialize the Java Access Bridge
        instance.initialize()?;
        
        Ok(instance)
    }
    
    /// Initialize the Java Access Bridge
    pub fn initialize(&mut self) -> Result<(), AutomationError> {
        if self.initialized {
            return Ok(());
        }
        
        info!("🔧 Initializing Java Access Bridge...");
        
        let success = unsafe { (self.initialize_access_bridge)() };
        
        if !success {
            return Err(AutomationError::PlatformError(
                "Failed to initialize Java Access Bridge".to_string(),
            ));
        }
        
        self.initialized = true;
        info!("✅ Java Access Bridge initialized successfully");
        
        // Set up event handlers
        self.setup_event_handlers()?;
        
        Ok(())
    }
    
    /// Set up event handlers for Java Access Bridge events
    fn setup_event_handlers(&self) -> Result<(), AutomationError> {
        debug!("Setting up Java Access Bridge event handlers...");
        
        // Set focus event handlers
        unsafe {
            (self.set_focus_gained)(Some(on_focus_gained));
            (self.set_focus_lost)(Some(on_focus_lost));
        }
        
        debug!("✅ Java Access Bridge event handlers set up");
        Ok(())
    }
    
    /// Check if a window handle belongs to a Java application
    pub fn is_java_window(&self, hwnd: *mut c_void) -> bool {
        if !self.initialized {
            return false;
        }
        
        unsafe { (self.is_java_window)(hwnd) }
    }
    
    /// Get accessible context from a window handle
    pub fn get_accessible_context_from_hwnd(
        &self,
        hwnd: *mut c_void,
    ) -> Result<(VmId, AccessibleContext), AutomationError> {
        if !self.initialized {
            return Err(AutomationError::PlatformError(
                "Java Access Bridge not initialized".to_string(),
            ));
        }
        
        let mut vm_id: VmId = 0;
        let mut ac: AccessibleContext = 0;
        
        let success = unsafe {
            (self.get_accessible_context_from_hwnd)(hwnd, &mut vm_id, &mut ac)
        };
        
        if !success {
            return Err(AutomationError::ElementNotFound(
                "Failed to get accessible context from window handle".to_string(),
            ));
        }
        
        Ok((vm_id, ac))
    }
    
    /// Get accessible context information
    pub fn get_accessible_context_info(
        &self,
        vm_id: VmId,
        ac: AccessibleContext,
    ) -> Result<AccessibleContextInfo, AutomationError> {
        if !self.initialized {
            return Err(AutomationError::PlatformError(
                "Java Access Bridge not initialized".to_string(),
            ));
        }
        
        let mut info = AccessibleContextInfo {
            name: [0; MAX_STRING_SIZE],
            description: [0; MAX_STRING_SIZE],
            role: [0; SHORT_STRING_SIZE],
            states: [0; SHORT_STRING_SIZE],
            index_in_parent: 0,
            children_count: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            accessible_component: false,
            accessible_action: false,
            accessible_selection: false,
            accessible_text: false,
            accessible_interfaces: false,
        };
        
        let success = unsafe {
            (self.get_accessible_context_info)(vm_id, ac, &mut info)
        };
        
        if !success {
            return Err(AutomationError::ElementNotFound(
                "Failed to get accessible context info".to_string(),
            ));
        }
        
        Ok(info)
    }
    
    /// Get child accessible context
    pub fn get_accessible_child_from_context(
        &self,
        vm_id: VmId,
        ac: AccessibleContext,
        index: i32,
    ) -> Result<AccessibleContext, AutomationError> {
        if !self.initialized {
            return Err(AutomationError::PlatformError(
                "Java Access Bridge not initialized".to_string(),
            ));
        }
        
        let child_ac = unsafe {
            (self.get_accessible_child_from_context)(vm_id, ac, index)
        };
        
        if child_ac == 0 {
            return Err(AutomationError::ElementNotFound(
                format!("Failed to get child at index {}", index),
            ));
        }
        
        Ok(child_ac)
    }
    
    /// Get parent accessible context
    pub fn get_accessible_parent_from_context(
        &self,
        vm_id: VmId,
        ac: AccessibleContext,
    ) -> Result<AccessibleContext, AutomationError> {
        if !self.initialized {
            return Err(AutomationError::PlatformError(
                "Java Access Bridge not initialized".to_string(),
            ));
        }
        
        let parent_ac = unsafe {
            (self.get_accessible_parent_from_context)(vm_id, ac)
        };
        
        if parent_ac == 0 {
            return Err(AutomationError::ElementNotFound(
                "Failed to get parent context".to_string(),
            ));
        }
        
        Ok(parent_ac)
    }
    
    /// Release Java object to prevent memory leaks
    pub fn release_java_object(&self, vm_id: VmId, object: JavaObject) {
        if self.initialized {
            unsafe {
                (self.release_java_object)(vm_id, object);
            }
        }
    }
    
    /// Get version information
    pub fn get_version_info(&self, vm_id: VmId) -> Result<AccessBridgeVersionInfo, AutomationError> {
        if !self.initialized {
            return Err(AutomationError::PlatformError(
                "Java Access Bridge not initialized".to_string(),
            ));
        }
        
        let mut info = AccessBridgeVersionInfo {
            vm_version: [0; SHORT_STRING_SIZE],
            bridge_java_class_version: [0; SHORT_STRING_SIZE],
            bridge_java_dll_version: [0; SHORT_STRING_SIZE],
            bridge_win_dll_version: [0; SHORT_STRING_SIZE],
        };
        
        let success = unsafe {
            (self.get_version_info)(vm_id, &mut info)
        };
        
        if !success {
            return Err(AutomationError::PlatformError(
                "Failed to get version info".to_string(),
            ));
        }
        
        Ok(info)
    }
}

impl Drop for JavaAccessBridge {
    fn drop(&mut self) {
        if self.initialized {
            info!("🔧 Shutting down Java Access Bridge...");
            let success = unsafe { (self.shutdown_access_bridge)() };
            if success {
                info!("✅ Java Access Bridge shut down successfully");
            } else {
                warn!("⚠️ Failed to properly shut down Java Access Bridge");
            }
        }
    }
}

// Event handler functions
unsafe extern "C" fn on_focus_gained(vm_id: VmId, event: JavaObject, source: JavaObject) {
    debug!("Java Access Bridge: Focus gained event - VM ID: {}, Event: {}, Source: {}", vm_id, event, source);
    // TODO: Implement focus gained handling
}

unsafe extern "C" fn on_focus_lost(vm_id: VmId, event: JavaObject, source: JavaObject) {
    debug!("Java Access Bridge: Focus lost event - VM ID: {}, Event: {}, Source: {}", vm_id, event, source);
    // TODO: Implement focus lost handling
}

/// Helper function to convert wide string to Rust String
pub fn wide_string_to_string(wide_str: &[u16]) -> String {
    let end = wide_str.iter().position(|&c| c == 0).unwrap_or(wide_str.len());
    String::from_utf16_lossy(&wide_str[..end])
}

/// Check if Java Access Bridge is available on the system
pub fn is_java_access_bridge_available() -> bool {
    match JavaAccessBridge::get_instance() {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_java_access_bridge_availability() {
        // This test will only pass if JAB is installed
        let available = is_java_access_bridge_available();
        println!("Java Access Bridge available: {}", available);
    }
    
    #[test]
    fn test_wide_string_conversion() {
        let wide_str = [72, 101, 108, 108, 111, 0]; // "Hello" in UTF-16
        let result = wide_string_to_string(&wide_str);
        assert_eq!(result, "Hello");
    }
}