const WS_URL = 'ws://127.0.0.1:17373';
let socket = null;
let reconnectTimer = null;

function log(...args) {
  console.log('[TerminatorBridge]', ...args);
}

function connect() {
  try {
    socket = new WebSocket(WS_URL);
  } catch (e) {
    log('WebSocket construct error', e);
    scheduleReconnect();
    return;
  }

  socket.onopen = () => {
    log('Connected to', WS_URL);
    socket.send(JSON.stringify({ type: 'hello', from: 'extension' }));
  };

  socket.onclose = () => {
    log('Socket closed');
    scheduleReconnect();
  };

  socket.onerror = (e) => {
    log('Socket error', e);
    try { socket.close(); } catch (_) {}
  };

  socket.onmessage = async (event) => {
    let msg;
    try {
      msg = JSON.parse(event.data);
    } catch (e) {
      log('Invalid JSON', event.data);
      return;
    }
    if (!msg || !msg.action) return;

    if (msg.action === 'eval') {
      const { id, code, awaitPromise = true } = msg;
      try {
        const tabId = await getActiveTabId();
        const result = await evalInTab(tabId, code, awaitPromise);
        safeSend({ id, ok: true, result });
      } catch (err) {
        safeSend({ id, ok: false, error: String(err && (err.message || err)) });
      }
    } else if (msg.action === 'ping') {
      safeSend({ type: 'pong' });
    }
  };
}

function scheduleReconnect() {
  if (reconnectTimer) return;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connect();
  }, 1500);
}

function safeSend(obj) {
  try {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(obj));
    }
  } catch (e) {
    log('Failed to send', e);
  }
}

async function getActiveTabId() {
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tab || tab.id == null) throw new Error('No active tab');
  return tab.id;
}

async function evalInTab(tabId, code, awaitPromise) {
  await debuggerAttach(tabId);
  try {
    await sendCommand(tabId, 'Runtime.enable', {});
    const { result, exceptionDetails } = await sendCommand(tabId, 'Runtime.evaluate', {
      expression: code,
      awaitPromise: !!awaitPromise,
      returnByValue: true,
      userGesture: true
    });
    if (exceptionDetails) {
      throw new Error(exceptionDetails.text || 'Evaluation error');
    }
    // Return JSON-serializable value
    return result?.value;
  } finally {
    try { await debuggerDetach(tabId); } catch (_) {}
  }
}

function debuggerAttach(tabId) {
  return new Promise((resolve, reject) => {
    chrome.debugger.attach({ tabId }, '1.3', (err) => {
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