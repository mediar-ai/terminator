const WS_URL = "ws://127.0.0.1:17373";
let socket = null;
let reconnectTimer = null;

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

function connect() {
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
    socket.send(JSON.stringify({ type: "hello", from: "extension" }));
  };

  socket.onclose = () => {
    log("Socket closed");
    scheduleReconnect();
  };

  socket.onerror = (e) => {
    log("Socket error", e);
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
        safeSend({ id, ok: true, result });
      } catch (err) {
        safeSend({ id, ok: false, error: String(err && (err.message || err)) });
      }
    } else if (msg.action === "ping") {
      safeSend({ type: "pong" });
    }
  };
}

function scheduleReconnect() {
  if (reconnectTimer) return;
  const delay = currentReconnectDelayMs;
  currentReconnectDelayMs = Math.min(
    currentReconnectDelayMs * 2,
    MAX_RECONNECT_DELAY_MS
  );
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    log(`Reconnecting... (delay=${delay}ms)`);
    connect();
  }, delay);
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

async function evalInTab(tabId, code, awaitPromise, evalId) {
  await debuggerAttach(tabId);
  let onEvent = null;
  try {
    await sendCommand(tabId, "Runtime.enable", {});
    await sendCommand(tabId, "Log.enable", {});

    // Listen for console/log/exception events for this tab while the eval runs
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
    const { result, exceptionDetails } = await sendCommand(
      tabId,
      "Runtime.evaluate",
      {
        expression: code,
        awaitPromise: !!awaitPromise,
        returnByValue: true,
        userGesture: true,
      }
    );
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
    // Return JSON-serializable value
    return result?.value;
  } finally {
    try {
      // Best-effort: remove onEvent listener and disable domains
      if (
        onEvent &&
        chrome &&
        chrome.debugger &&
        chrome.debugger.onEvent &&
        chrome.debugger.onEvent.removeListener
      ) {
        chrome.debugger.onEvent.removeListener(onEvent);
      }
      try {
        await sendCommand(tabId, "Log.disable", {});
      } catch (_) {}
      try {
        await sendCommand(tabId, "Runtime.disable", {});
      } catch (_) {}
    } catch (_) {}
    try {
      await debuggerDetach(tabId);
    } catch (_) {}
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

connect();
