const WS_URL = "ws://127.0.0.1:17373";
let socket = null;
let reconnectTimer = null;

// Simple Set to track what we've attached to
const attachedTabs = new Set();
// Track which tabs have Runtime and Log domains enabled
const enabledTabs = new Set();
// NEW: Track which tabs have Network domain enabled
const networkEnabledTabs = new Set();
// NEW: Track persistent network listeners (one per tab)
const activeNetworkListeners = new Map(); // Map<tabId, listenerFunction>

// Clear stored tabs on startup since debugger sessions don't persist across restarts
chrome.storage.session.remove('attached').then(() => {
  log('Cleared stale debugger session data on startup');
});

// Logging is disabled by default to reduce console noise. Toggle via Service Worker console.
let debugEnabled = true;
self.enableTerminatorDebug = () => {
  debugEnabled = true;
};
self.disableTerminatorDebug = () => {
  debugEnabled = false;
};
function log(...args) {
  if (!debugEnabled) return;
  console.log("[TerminatorBridge]", ...args);
}

// Exponential backoff to reduce repeated connection error spam
const BASE_RECONNECT_DELAY_MS = 500; // faster initial retry
const MAX_RECONNECT_DELAY_MS = 3000; // cap retries to 3s to align with host waiting
let currentReconnectDelayMs = BASE_RECONNECT_DELAY_MS;
let connectionAttempts = 0;
const MAX_LOGGED_ATTEMPTS = 3; // Only log first few attempts to reduce noise

function connect() {
  try {
    if (
      socket &&
      (socket.readyState === WebSocket.OPEN ||
        socket.readyState === WebSocket.CONNECTING)
    ) {
      return;
    }
  } catch (_) {}
  try {
    socket = new WebSocket(WS_URL);
  } catch (e) {
    log("WebSocket construct error", e);
    scheduleReconnect();
    return;
  }

  socket.onopen = () => {
    log("Connected to", WS_URL);
    // Reset backoff on successful connection
    currentReconnectDelayMs = BASE_RECONNECT_DELAY_MS;
    connectionAttempts = 0; // Reset connection attempts on successful connection
    socket.send(JSON.stringify({ type: "hello", from: "extension" }));
  };

  socket.onclose = () => {
    // Only log socket closed for first 3 attempts and every 10th
    if (connectionAttempts <= 3 || connectionAttempts % 10 === 0) {
      log(`Socket closed (attempt ${connectionAttempts})`);
    }
    scheduleReconnect();
  };

  socket.onerror = (e) => {
    connectionAttempts++;
    // Only log first 3 attempts and then every 10th attempt to reduce noise
    if (connectionAttempts <= 3 || connectionAttempts % 10 === 0) {
      log(`Socket error (attempt ${connectionAttempts})`, e);
    }
    try {
      socket.close();
    } catch (_) {}
  };

  socket.onmessage = async (event) => {
    let msg;
    try {
      msg = JSON.parse(event.data);
    } catch (e) {
      log("Invalid JSON", event.data);
      return;
    }
    if (!msg || !msg.action) return;

    if (msg.action === "eval") {
      const { id, code, awaitPromise = true } = msg;
      try {
        const tabId = await getActiveTabId();
        const result = await evalInTab(tabId, code, awaitPromise, id);
        safeSend({ id, ok: true, result, tabId });
      } catch (err) {
        safeSend({ id, ok: false, error: String(err && (err.message || err)) });
      }
    } else if (msg.action === "ping") {
      safeSend({ type: "pong" });
    } else if (msg.action === "reset") {
      // Force reset all debugger state
      log("Received reset command");
      await forceResetDebuggerState();
      safeSend({ type: "reset_complete", ok: true });
    } else if (msg.action === "capture_element_at_point") {
      // New action for recording DOM elements
      const { id, x, y } = msg;
      try {
        const tabId = await getActiveTabId();
        const result = await captureElementAtPoint(tabId, x, y, id);
        safeSend({ id, ok: true, result });
      } catch (err) {
        safeSend({ id, ok: false, error: String(err && (err.message || err)) });
      }
    } else if (msg.action === "start_recording_session") {
      // Initialize recording mode
      const { id, sessionId } = msg;
      try {
        await startRecordingSession(sessionId);
        safeSend({ id, ok: true, result: { sessionId, status: "recording" } });
      } catch (err) {
        safeSend({ id, ok: false, error: String(err && (err.message || err)) });
      }
    } else if (msg.action === "stop_recording_session") {
      // Stop recording mode
      const { id, sessionId } = msg;
      try {
        await stopRecordingSession(sessionId);
        safeSend({ id, ok: true, result: { sessionId, status: "stopped" } });
      } catch (err) {
        safeSend({ id, ok: false, error: String(err && (err.message || err)) });
      }
    }
  };
}

function ensureConnected() {
  try {
    if (
      !socket ||
      (socket.readyState !== WebSocket.OPEN &&
        socket.readyState !== WebSocket.CONNECTING)
    ) {
      connect();
    }
  } catch (_) {
    connect();
  }
}

// Ensure we have an active WS connection on first message from content script
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  try {
    if (!message || message.type !== "terminator_content_handshake") return;
    log("Received handshake from content script", {
      tab: sender.tab && sender.tab.id,
    });
    // Kick the connector if not already connected; otherwise noop
    ensureConnected();
    sendResponse({ ok: true });
  } catch (e) {
    try {
      sendResponse({ ok: false, error: String(e && (e.message || e)) });
    } catch (_) {}
    // swallow
  }
  // Keep listener alive for async sendResponse
  return true;
});

// Inject a lightweight handshake into the active tab when it changes/updates
async function injectHandshake(tabId) {
  try {
    if (typeof tabId !== "number") return;
    await chrome.scripting.executeScript({
      target: { tabId },
      func: () => {
        try {
          chrome.runtime.sendMessage({ type: "terminator_content_handshake" });
        } catch (_) {}
      },
      world: "ISOLATED",
    });
  } catch (e) {
    // ignore (e.g., not permitted on special pages)
  }
}

async function getActiveTabIdSafe() {
  try {
    const [tab] = await chrome.tabs.query({
      active: true,
      lastFocusedWindow: true,
    });
    return tab && tab.id != null ? tab.id : null;
  } catch (_) {
    return null;
  }
}

// Additional event-based triggers to wake the worker and maintain connection
chrome.runtime.onInstalled.addListener(() => {
  log("onInstalled → ensureConnected");
  ensureConnected();
});
chrome.runtime.onStartup.addListener(() => {
  log("onStartup → ensureConnected");
  ensureConnected();
});
chrome.webNavigation.onCommitted.addListener(() => {
  ensureConnected();
});
chrome.alarms.clear("terminator_keepalive");
chrome.alarms.create("terminator_keepalive", { periodInMinutes: 1 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm && alarm.name === "terminator_keepalive") {
    ensureConnected();
  }
});
chrome.tabs.onActivated.addListener(async () => {
  ensureConnected();
  const tabId = await getActiveTabIdSafe();
  if (tabId != null) injectHandshake(tabId);
});
chrome.tabs.onUpdated.addListener(async (tabId, changeInfo) => {
  if (
    changeInfo &&
    (changeInfo.status === "loading" || changeInfo.status === "complete")
  ) {
    ensureConnected();
    injectHandshake(tabId);
  }
});

// Clean up our tracking when tab closes
chrome.tabs.onRemoved.addListener(tabId => {
  if (attachedTabs.has(tabId)) {
    attachedTabs.delete(tabId);
    enabledTabs.delete(tabId);  // Also clean enabled domains state
    networkEnabledTabs.delete(tabId);  // NEW: Clean network domain state

    // NEW: Remove persistent network listener
    const networkListener = activeNetworkListeners.get(tabId);
    if (networkListener) {
      chrome.debugger.onEvent.removeListener(networkListener);
      activeNetworkListeners.delete(tabId);
      log(`Removed persistent network listener for tab ${tabId}`);
    }

    chrome.storage.session.set({attached: [...attachedTabs]});
    log(`Tab ${tabId} closed, removed from attached tabs and enabled domains`);
  }
});

// Clean up when debugger is manually detached (user clicked Cancel)
chrome.debugger.onDetach.addListener((source, reason) => {
  const tabId = source.tabId;
  if (attachedTabs.has(tabId)) {
    attachedTabs.delete(tabId);
    enabledTabs.delete(tabId);  // Also clean enabled domains state
    networkEnabledTabs.delete(tabId);  // NEW: Clean network domain state

    // NEW: Remove persistent network listener
    const networkListener = activeNetworkListeners.get(tabId);
    if (networkListener) {
      chrome.debugger.onEvent.removeListener(networkListener);
      activeNetworkListeners.delete(tabId);
      log(`Removed persistent network listener for tab ${tabId}`);
    }

    chrome.storage.session.set({attached: [...attachedTabs]});
    log(`Debugger detached from tab ${tabId}, reason: ${reason}`);
  }
});

function scheduleReconnect() {
  if (reconnectTimer) return;

  const delay = currentReconnectDelayMs;
  currentReconnectDelayMs = Math.min(
    currentReconnectDelayMs * 2,
    MAX_RECONNECT_DELAY_MS
  );
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    // Only log reconnection for first 3 attempts and then every 10th
    if (connectionAttempts <= 3 || connectionAttempts % 10 === 0) {
      log(`Reconnecting... (attempt ${connectionAttempts + 1}, delay=${delay}ms)`);
    }
    connect();
  }, delay);
}

async function forceResetDebuggerState() {
  log("Force resetting all debugger state...");

  // Store the previous size for logging
  const previousSize = attachedTabs.size;

  // 1. FIRST: Detach from all tabs (must happen before clearing state)
  let detachedCount = 0;
  try {
    const tabs = await chrome.tabs.query({});
    for (const tab of tabs) {
      if (tab.id != null) {
        try {
          await debuggerDetach(tab.id);
          detachedCount++;
        } catch (e) {
          // Ignore - tab might not be attached or might be special page
        }
      }
    }
  } catch (e) {
    log("Error during tab cleanup (continuing anyway):", e.message || e);
  }

  // 2. THEN: Clear in-memory state (after detachment completes)
  attachedTabs.clear();
  enabledTabs.clear();
  networkEnabledTabs.clear();  // NEW: Clear network domain state

  // NEW: Remove all persistent network listeners
  for (const [tabId, networkListener] of activeNetworkListeners.entries()) {
    try {
      chrome.debugger.onEvent.removeListener(networkListener);
      log(`Removed persistent network listener for tab ${tabId} during reset`);
    } catch (e) {
      // Ignore - listener might already be removed
    }
  }
  activeNetworkListeners.clear();

  // 3. Clear session storage
  await chrome.storage.session.remove('attached');

  log(`Reset complete: cleared ${previousSize} tracked tabs, detached from ${detachedCount} tabs`);
  log("Debugger state reset complete");
}

function safeSend(obj) {
  try {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(obj));
    }
  } catch (e) {
    log("Failed to send", e);
  }
}

async function getActiveTabId() {
  const [tab] = await chrome.tabs.query({
    active: true,
    lastFocusedWindow: true,
  });
  if (!tab || tab.id == null) throw new Error("No active tab");
  return tab.id;
}

function formatRemoteObject(obj) {
  try {
    if (obj === null || obj === undefined) return null;
    if (Object.prototype.hasOwnProperty.call(obj, "value")) return obj.value;
    if (obj.description !== undefined) return obj.description;
    return obj.type || null;
  } catch (_) {
    return null;
  }
}

// Helper function to detect if code has top-level return statements
function hasTopLevelReturn(code) {
  // Quick check if 'return' keyword exists at all
  if (!code.includes('return')) {
    return false;
  }

  // Simple heuristic: if code starts with return or has return after newline/semicolon
  // but NOT inside a function body
  // This is a simplified check that covers most common cases

  // First, check for some patterns that definitely DON'T need wrapping
  // 1. Already wrapped in IIFE
  if (/^\s*\(\s*function\s*\(/.test(code) || /^\s*\(\s*\(\s*\)/.test(code)) {
    return false;
  }

  // 2. Is just an expression (no statements)
  if (!code.includes(';') && !code.includes('\n') && !code.includes('return')) {
    return false;
  }

  // Remove strings and comments for cleaner analysis
  let cleanCode = code
    // Remove single-line comments
    .replace(/\/\/.*$/gm, '')
    // Remove multi-line comments
    .replace(/\/\*[\s\S]*?\*\//g, '')
    // Remove strings (simplified - doesn't handle escaped quotes)
    .replace(/"[^"]*"/g, '""')
    .replace(/'[^']*'/g, "''")
    .replace(/`[^`]*`/g, '``');

  // Look for return statements that are likely at the top level
  // Common patterns:
  // 1. return at start of code (with optional whitespace)
  // 2. return after a semicolon or closing brace
  // 3. return on a new line
  const returnPatterns = [
    /^\s*return\s+/,           // starts with return
    /;\s*return\s+/,            // return after semicolon
    /}\s*return\s+/,            // return after closing brace (like after if block)
    /\n\s*return\s+/            // return on new line
  ];

  // Check if any of these patterns exist
  const hasReturn = returnPatterns.some(pattern => pattern.test(cleanCode));

  if (!hasReturn) {
    return false;
  }

  // Additional check: if it looks like it's inside a function, don't wrap
  // Look for function keyword before the return
  const beforeReturn = cleanCode.substring(0, cleanCode.indexOf('return'));

  // Count unmatched opening braces before return
  const openBraces = (beforeReturn.match(/{/g) || []).length;
  const closeBraces = (beforeReturn.match(/}/g) || []).length;

  // If we have unclosed braces and a function declaration, the return is likely inside it
  if (openBraces > closeBraces && /function\s*\(|=>\s*{/.test(beforeReturn)) {
    return false;
  }

  // Likely has top-level return
  return true;
}

// Helper function to detect async IIFE pattern
function isAsyncIIFE(code) {
  // Extract the last statement from the code (after any var declarations)
  const trimmed = code.trim();

  // Find the last complete statement by looking for the last IIFE pattern
  // This handles cases where env vars are injected before the IIFE
  const asyncIIFEPatterns = [
    /\(\s*async\s+function\s*\([^)]*\)\s*{[\s\S]*?}\s*\)\s*\(\s*\)\s*;?\s*$/,
    /\(\s*async\s*\([^)]*\)\s*=>\s*{[\s\S]*?}\s*\)\s*\(\s*\)\s*;?\s*$/,
    /\(\s*async\s+function\s+\w+\s*\([^)]*\)\s*{[\s\S]*?}\s*\)\s*\(\s*\)\s*;?\s*$/
  ];

  return asyncIIFEPatterns.some(pattern => pattern.test(trimmed));
}

// Helper function to detect regular (non-async) IIFE pattern
function isRegularIIFE(code) {
  // Extract the last statement from the code (after any var declarations)
  const trimmed = code.trim();

  // Find the last complete statement by looking for the last IIFE pattern
  // This handles cases where env vars are injected before the IIFE
  const iifePatterns = [
    /\(\s*function\s*\([^)]*\)\s*{[\s\S]*?}\s*\)\s*\(\s*\)\s*;?\s*$/,
    /\(\s*function\s+\w+\s*\([^)]*\)\s*{[\s\S]*?}\s*\)\s*\(\s*\)\s*;?\s*$/,
    /\(\s*\([^)]*\)\s*=>\s*{[\s\S]*?}\s*\)\s*\(\s*\)\s*;?\s*$/
  ];

  return iifePatterns.some(pattern => pattern.test(trimmed));
}

// Helper function to wrap code in IIFE if it has top-level returns
function wrapCodeIfNeeded(code) {
  // Check if it's an async IIFE - if so, return as-is since it's already wrapped
  if (isAsyncIIFE(code)) {
    log("Detected async IIFE pattern, using as-is (will set awaitPromise=true)");
    return code;
  }

  // Check if it's a regular IIFE - if so, return as-is since it's already wrapped
  if (isRegularIIFE(code)) {
    log("Detected regular IIFE pattern, using as-is");
    return code;
  }

  if (hasTopLevelReturn(code)) {
    log("Detected top-level return statement, wrapping in IIFE");
    return `(function() {\n${code}\n})()`;
  }

  // For code without top-level returns, wrap in eval to capture last expression
  // This creates a clean scope while still returning the last expression
  log("Wrapping in eval to capture last expression and create clean scope");
  return `(function() { return eval(${JSON.stringify(code)}); })()`;
}

async function evalInTab(tabId, code, awaitPromise, evalId) {
  const perfStart = performance.now();
  const timings = {};

  // Auto-detect async IIFEs and set awaitPromise accordingly
  const originalCode = code;
  if (isAsyncIIFE(originalCode)) {
    log("Detected async IIFE, forcing awaitPromise=true");
    awaitPromise = true;
  }

  // Auto-detect and wrap code with top-level returns
  code = wrapCodeIfNeeded(code);

  // Only attach if we haven't before
  if (!attachedTabs.has(tabId)) {
    const attachStart = performance.now();
    try {
      await debuggerAttach(tabId);
      attachedTabs.add(tabId);
      // Persist (fire and forget)
      chrome.storage.session.set({attached: [...attachedTabs]});
      timings.attach = performance.now() - attachStart;
      log(`Debugger attached to tab ${tabId} (${timings.attach.toFixed(1)}ms)`);
    } catch (e) {
      // If already attached by another concurrent operation, treat as attached
      if (e.message && e.message.includes("already attached")) {
        log(`Tab ${tabId} already attached by another operation, treating as success`);
        attachedTabs.add(tabId);
        chrome.storage.session.set({attached: [...attachedTabs]});
        timings.attach = performance.now() - attachStart;
      } else {
        // If we can't attach, we can't continue - throw the error
        log(`Could not attach to tab ${tabId}:`, e.message);
        throw new Error(`Failed to attach debugger: ${e.message}`);
      }
    }
  } else {
    // Verify the debugger is actually still attached
    try {
      // Try a simple command to verify connection
      await sendCommand(tabId, "Runtime.evaluate", { expression: "1", returnByValue: true });
      log(`Reusing existing debugger for tab ${tabId}`);
      timings.attach = 0;
    } catch (e) {
      // Debugger was detached, need to reattach
      log(`Debugger was detached from tab ${tabId}, reattaching...`);
      attachedTabs.delete(tabId);
      enabledTabs.delete(tabId);

      const attachStart = performance.now();
      try {
        await debuggerAttach(tabId);
        attachedTabs.add(tabId);
        chrome.storage.session.set({attached: [...attachedTabs]});
        timings.attach = performance.now() - attachStart;
        log(`Debugger reattached to tab ${tabId} (${timings.attach.toFixed(1)}ms)`);
      } catch (attachError) {
        // If already attached by another concurrent operation, treat as attached
        if (attachError.message && attachError.message.includes("already attached")) {
          log(`Tab ${tabId} already attached by another operation, treating as success`);
          attachedTabs.add(tabId);
          chrome.storage.session.set({attached: [...attachedTabs]});
          timings.attach = performance.now() - attachStart;
        } else {
          log(`Failed to reattach to tab ${tabId}:`, attachError.message);
          throw new Error(`Failed to reattach debugger: ${attachError.message}`);
        }
      }
    }
  }
  
  let onEvent = null;
  try {
    // Only enable domains if not already enabled
    if (!enabledTabs.has(tabId)) {
      const enableStart = performance.now();
      try {
        const runtimeStart = performance.now();
        await sendCommand(tabId, "Runtime.enable", {});
        timings.runtimeEnable = performance.now() - runtimeStart;

        const logStart = performance.now();
        await sendCommand(tabId, "Log.enable", {});
        timings.logEnable = performance.now() - logStart;

        enabledTabs.add(tabId);
        timings.totalEnable = performance.now() - enableStart;
        log(`Domains enabled for tab ${tabId} - Runtime: ${timings.runtimeEnable.toFixed(1)}ms, Log: ${timings.logEnable.toFixed(1)}ms, Total: ${timings.totalEnable.toFixed(1)}ms`);
      } catch (e) {
        log(`Could not enable domains for tab ${tabId}:`, e.message);
        // Clear the tab from tracking since it's in a bad state
        attachedTabs.delete(tabId);
        enabledTabs.delete(tabId);
        // Try to detach and clean up
        try {
          await debuggerDetach(tabId);
        } catch (_) {}
        // Throw the error to trigger retry logic at the higher level
        throw new Error(`Failed to enable debugger domains: ${e.message}`);
      }
    } else {
      log(`Reusing enabled domains for tab ${tabId}`);
      timings.runtimeEnable = 0;
      timings.logEnable = 0;
      timings.totalEnable = 0;
    }

    // NEW: Enable Network domain for network request capture and create persistent listener
    if (!networkEnabledTabs.has(tabId)) {
      const networkStart = performance.now();
      try {
        await sendCommand(tabId, "Network.enable", {});
        timings.networkEnable = performance.now() - networkStart;
        networkEnabledTabs.add(tabId);
        log(`Network domain enabled for tab ${tabId} (${timings.networkEnable.toFixed(1)}ms)`);

        // Create persistent network listener for this tab (one per tab, not per script)
        const networkListener = (source, method, params) => {
          try {
            if (!source || source.tabId !== tabId) return;

            if (method === "Network.requestWillBeSent") {
              safeSend({
                type: "network_request",
                tabId: tabId,
                requestId: params.requestId,
                request: {
                  url: params.request.url,
                  method: params.request.method,
                  headers: params.request.headers,
                  postData: params.request.postData,
                  hasPostData: params.request.hasPostData,
                  initialPriority: params.request.initialPriority,
                  referrerPolicy: params.request.referrerPolicy,
                },
                documentURL: params.documentURL,
                timestamp: params.timestamp,
                wallTime: params.wallTime,
                initiator: params.initiator,
                resourceType: params.type,
                frameId: params.frameId,
                redirectResponse: params.redirectResponse ? {
                  url: params.redirectResponse.url,
                  status: params.redirectResponse.status,
                  statusText: params.redirectResponse.statusText,
                } : null
              });
            } else if (method === "Network.responseReceived") {
              safeSend({
                type: "network_response",
                tabId: tabId,
                requestId: params.requestId,
                response: {
                  url: params.response.url,
                  status: params.response.status,
                  statusText: params.response.statusText,
                  headers: params.response.headers,
                  mimeType: params.response.mimeType,
                  requestHeaders: params.response.requestHeaders,
                  connectionReused: params.response.connectionReused,
                  connectionId: params.response.connectionId,
                  remoteIPAddress: params.response.remoteIPAddress,
                  remotePort: params.response.remotePort,
                  fromDiskCache: params.response.fromDiskCache,
                  fromServiceWorker: params.response.fromServiceWorker,
                  fromPrefetchCache: params.response.fromPrefetchCache,
                  encodedDataLength: params.response.encodedDataLength,
                  timing: params.response.timing,
                  protocol: params.response.protocol,
                  securityState: params.response.securityState,
                },
                timestamp: params.timestamp,
                resourceType: params.type,
                frameId: params.frameId
              });
            }
          } catch (e) {
            // swallow
          }
        };

        // Add persistent network listener (will remain active for tab lifetime)
        chrome.debugger.onEvent.addListener(networkListener);
        activeNetworkListeners.set(tabId, networkListener);
        log(`Persistent network listener added for tab ${tabId}`);
      } catch (e) {
        log(`Could not enable Network domain for tab ${tabId}:`, e.message);
        // Network domain is optional - don't fail if it can't be enabled
        timings.networkEnable = 0;
      }
    } else {
      log(`Reusing enabled Network domain for tab ${tabId}`);
      timings.networkEnable = 0;
    }

    // Listen for console/log/exception events for this tab while the eval runs
    // NOTE: Network events are now handled by the persistent networkListener above
    onEvent = (source, method, params) => {
      try {
        if (!source || source.tabId !== tabId) return;
        if (method === "Runtime.consoleAPICalled") {
          const level = params.type || "log";
          const args = (params.args || []).map((a) => formatRemoteObject(a));
          const stackTrace = params.stackTrace || null;
          safeSend({
            type: "console_event",
            id: evalId,
            level,
            args,
            stackTrace,
            ts: params.timestamp || Date.now(),
          });
        } else if (method === "Runtime.exceptionThrown") {
          safeSend({
            type: "exception_event",
            id: evalId,
            details: params.exceptionDetails || params || null,
          });
        } else if (method === "Log.entryAdded") {
          safeSend({
            type: "log_event",
            id: evalId,
            entry: params.entry || params || null,
          });
        }
      } catch (e) {
        // swallow
      }
    };
    chrome.debugger.onEvent.addListener(onEvent);
    
    const evalStart = performance.now();
    let evalResult;
    let retryWithWrapper = false;

    try {
      evalResult = await sendCommand(
        tabId,
        "Runtime.evaluate",
        {
          expression: code,
          awaitPromise: !!awaitPromise,
          returnByValue: true,
          userGesture: true,
        }
      );
    } catch (error) {
      // If sendCommand itself fails (not script error), re-throw
      throw error;
    }

    let { result, exceptionDetails } = evalResult;
    timings.evaluate = performance.now() - evalStart;

    // Check if we got "Illegal return statement" error and haven't wrapped yet
    if (exceptionDetails &&
        code !== originalCode && // Already wrapped, don't retry
        (exceptionDetails.text?.includes("Illegal return statement") ||
         exceptionDetails.exception?.description?.includes("Illegal return statement"))) {
      // This shouldn't happen since we already wrapped, but log it
      log("Still got 'Illegal return statement' after wrapping, not retrying");
    } else if (exceptionDetails &&
               code === originalCode && // Not wrapped yet
               (exceptionDetails.text?.includes("Illegal return statement") ||
                exceptionDetails.exception?.description?.includes("Illegal return statement"))) {
      // Our detection missed it, retry with wrapper
      log("Got 'Illegal return statement' error, retrying with IIFE wrapper");
      retryWithWrapper = true;
    }

    // Retry with wrapper if needed
    if (retryWithWrapper) {
      const wrappedCode = `(function() {\n${originalCode}\n})()`;
      const retryStart = performance.now();

      evalResult = await sendCommand(
        tabId,
        "Runtime.evaluate",
        {
          expression: wrappedCode,
          awaitPromise: !!awaitPromise,
          returnByValue: true,
          userGesture: true,
        }
      );

      ({ result, exceptionDetails } = evalResult);
      timings.evaluateRetry = performance.now() - retryStart;
      timings.evaluate += timings.evaluateRetry;
      log(`Retry with wrapper took ${timings.evaluateRetry.toFixed(1)}ms`);
    }

    if (exceptionDetails) {
      // Build rich error details for MCP side
      const details = {
        text: exceptionDetails.text,
        url: exceptionDetails.url,
        lineNumber: exceptionDetails.lineNumber,
        columnNumber: exceptionDetails.columnNumber,
        exception:
          (exceptionDetails.exception &&
            (exceptionDetails.exception.description ||
              exceptionDetails.exception.value)) ||
          null,
        stackTrace:
          (exceptionDetails.stackTrace &&
            Array.isArray(exceptionDetails.stackTrace.callFrames) &&
            exceptionDetails.stackTrace.callFrames.map((cf) => ({
              functionName: cf.functionName,
              url: cf.url,
              lineNumber: cf.lineNumber,
              columnNumber: cf.columnNumber,
            }))) ||
          null,
      };
      // Emit console error for visibility in extension worker logs
      console.error("[TerminatorBridge] Eval exception:", details);
      // Throw a JSON-encoded error so the bridge returns full context
      throw new Error(
        JSON.stringify({
          code: "EVAL_ERROR",
          message: details.text || "Evaluation error",
          details,
        })
      );
    }
    
    // Log timing summary
    timings.total = performance.now() - perfStart;
    log(`[TIMING] Total: ${timings.total.toFixed(1)}ms | Attach: ${timings.attach.toFixed(1)}ms | Enable: ${timings.totalEnable.toFixed(1)}ms | Eval: ${timings.evaluate.toFixed(1)}ms`);

    // Return JSON-serializable value
    // Check if result is null or undefined - these should be treated as errors
    const resultValue = result?.value;
    if (resultValue === null || resultValue === undefined) {
      // Throw an error to trigger workflow fallback_id behavior
      throw new Error(
        JSON.stringify({
          code: "NULL_RESULT",
          message: "JavaScript execution returned null or undefined",
          details: {
            text: "Script returned null/undefined value",
            resultType: resultValue === null ? "null" : "undefined"
          }
        })
      );
    }
    return resultValue;
  } finally {
    try {
      // Best-effort: remove onEvent listener
      if (
        onEvent &&
        chrome &&
        chrome.debugger &&
        chrome.debugger.onEvent &&
        chrome.debugger.onEvent.removeListener
      ) {
        chrome.debugger.onEvent.removeListener(onEvent);
      }
      // DON'T DISABLE DOMAINS - Keep them enabled for reuse
      // Removed: await sendCommand(tabId, "Log.disable", {});
      // Removed: await sendCommand(tabId, "Runtime.disable", {});
    } catch (_) {}
    // DON'T DETACH - Keep debugger attached for reuse
    // This was: await debuggerDetach(tabId);
  }
}

function debuggerAttach(tabId) {
  return new Promise((resolve, reject) => {
    chrome.debugger.attach({ tabId }, "1.3", (err) => {
      if (chrome.runtime.lastError) return reject(chrome.runtime.lastError);
      resolve();
    });
  });
}

function debuggerDetach(tabId) {
  return new Promise((resolve) => {
    chrome.debugger.detach({ tabId }, () => resolve());
  });
}

function sendCommand(tabId, method, params) {
  return new Promise((resolve, reject) => {
    chrome.debugger.sendCommand({ tabId }, method, params, (result) => {
      const err = chrome.runtime.lastError;
      if (err) return reject(err);
      resolve(result || {});
    });
  });
}

// Recording session management
let recordingSessionId = null;

async function startRecordingSession(sessionId) {
  recordingSessionId = sessionId;
  log(`Started recording session: ${sessionId}`);
  // Could inject content scripts here if needed
  return { sessionId, status: "recording" };
}

async function stopRecordingSession(sessionId) {
  if (recordingSessionId === sessionId) {
    recordingSessionId = null;
    log(`Stopped recording session: ${sessionId}`);
  }
  return { sessionId, status: "stopped" };
}

// DOM element capture for recording
async function captureElementAtPoint(tabId, x, y, captureId) {
  const code = `(function() {
    const x = ${x};
    const y = ${y};

    // Get element at coordinates
    const element = document.elementFromPoint(x, y);
    if (!element) {
      return { error: 'No element at coordinates', x: x, y: y };
    }

    // Generate selector candidates
    function generateSelectors(el) {
      const selectors = [];

      // 1. ID selector (highest priority)
      if (el.id) {
        selectors.push({
          selector: '#' + CSS.escape(el.id),
          selector_type: 'Id',
          specificity: 100,
          requires_jquery: false
        });
      }

      // 2. Data attributes
      const dataAttrs = Array.from(el.attributes)
        .filter(attr => attr.name.startsWith('data-'))
        .map(attr => ({
          selector: '[' + attr.name + '="' + CSS.escape(attr.value) + '"]',
          selector_type: 'DataAttribute',
          specificity: 90,
          requires_jquery: false
        }));
      selectors.push(...dataAttrs);

      // 3. Aria label
      if (el.getAttribute('aria-label')) {
        selectors.push({
          selector: '[aria-label="' + CSS.escape(el.getAttribute('aria-label')) + '"]',
          selector_type: 'AriaLabel',
          specificity: 85,
          requires_jquery: false
        });
      }

      // 4. Class combinations
      if (el.className && typeof el.className === 'string') {
        const classes = el.className.split(' ').filter(c => c);
        if (classes.length > 0) {
          selectors.push({
            selector: '.' + classes.map(c => CSS.escape(c)).join('.'),
            selector_type: 'Class',
            specificity: 70,
            requires_jquery: false
          });
        }
      }

      // 5. Text content for buttons/links
      if (['button', 'a'].includes(el.tagName.toLowerCase())) {
        const text = el.textContent.trim();
        if (text && text.length < 50) {
          selectors.push({
            selector: el.tagName.toLowerCase() + ':contains("' + text + '")',
            selector_type: 'Text',
            specificity: 60,
            requires_jquery: true
          });
        }
      }

      // 6. Generate XPath
      function getXPath(element) {
        if (element.id) {
          return '//*[@id="' + element.id + '"]';
        }

        const parts = [];
        while (element && element.nodeType === Node.ELEMENT_NODE) {
          let index = 1;
          let sibling = element.previousElementSibling;
          while (sibling) {
            if (sibling.tagName === element.tagName) index++;
            sibling = sibling.previousElementSibling;
          }
          const tagName = element.tagName.toLowerCase();
          const part = tagName + '[' + index + ']';
          parts.unshift(part);
          element = element.parentElement;
        }
        return '/' + parts.join('/');
      }

      selectors.push({
        selector: getXPath(el),
        selector_type: 'XPath',
        specificity: 40,
        requires_jquery: false
      });

      // 7. CSS path (most specific, least maintainable)
      function getCSSPath(el) {
        const path = [];
        while (el && el.nodeType === Node.ELEMENT_NODE) {
          let selector = el.tagName.toLowerCase();
          if (el.id) {
            selector = '#' + CSS.escape(el.id);
            path.unshift(selector);
            break;
          } else if (el.className && typeof el.className === 'string') {
            const classes = el.className.split(' ').filter(c => c);
            if (classes.length > 0) {
              selector += '.' + classes.map(c => CSS.escape(c)).join('.');
            }
          }
          path.unshift(selector);
          el = el.parentElement;
        }
        return path.join(' > ');
      }

      selectors.push({
        selector: getCSSPath(el),
        selector_type: 'CssPath',
        specificity: 30,
        requires_jquery: false
      });

      return selectors;
    }

    // Capture element information
    const rect = element.getBoundingClientRect();
    const computedStyle = window.getComputedStyle(element);

    // Get all attributes as a map
    const attributes = {};
    for (const attr of element.attributes) {
      attributes[attr.name] = attr.value;
    }

    // Get class names as array
    const classNames = element.className
      ? (typeof element.className === 'string'
          ? element.className.split(' ').filter(c => c)
          : [])
      : [];

    return {
      tag_name: element.tagName.toLowerCase(),
      id: element.id || null,
      class_names: classNames,
      attributes: attributes,
      css_selector: getCSSPath(element),
      xpath: getXPath(element),
      inner_text: element.innerText ? element.innerText.substring(0, 100) : null,
      input_value: element.value || null,
      bounding_rect: {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        top: rect.top,
        left: rect.left
      },
      is_visible: computedStyle.display !== 'none' &&
                 computedStyle.visibility !== 'hidden' &&
                 computedStyle.opacity !== '0',
      is_interactive: !element.disabled &&
                     computedStyle.pointerEvents !== 'none',
      computed_role: element.getAttribute('role') || null,
      aria_label: element.getAttribute('aria-label') || null,
      placeholder: element.placeholder || null,
      selector_candidates: generateSelectors(element),
      page_context: {
        url: window.location.href,
        title: document.title,
        domain: window.location.hostname
      },
      capture_id: '${captureId}'
    };
  })()`;

  try {
    const result = await evalInTab(tabId, code, false, captureId);
    log(`Captured DOM element at (${x}, ${y}):`, result);
    return result;
  } catch (err) {
    log(`Failed to capture DOM element at (${x}, ${y}):`, err);
    throw err;
  }
}

connect();
