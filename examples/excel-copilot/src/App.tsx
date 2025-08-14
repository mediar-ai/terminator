import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';

interface ToolCall {
  function_name: string;
  arguments: any;
  result: string;
}

interface ChatMessage {
  role: string;
  content: string;
  timestamp: number;
  tool_calls?: ToolCall[];
  response_details?: {
    has_tool_calls: boolean;
    iterations: number;
  };
}

function App() {
  const [currentFile, setCurrentFile] = useState<string>('');
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [currentMessage, setCurrentMessage] = useState('');
  const [isGeminiConfigured, setIsGeminiConfigured] = useState(false);
  const [isOpenAIConfigured, setIsOpenAIConfigured] = useState(false);
  const [geminiApiKey, setGeminiApiKey] = useState('');
  const [openaiApiKey, setOpenaiApiKey] = useState('');
  const [selectedLlm, setSelectedLlm] = useState<'gemini' | 'openai'>('gemini');
  const [showApiKeyInput, setShowApiKeyInput] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [status, setStatus] = useState('ready');
  const [attachedPdfs, setAttachedPdfs] = useState<string[]>([]);
  const [copilotEnabled, setCopilotEnabled] = useState(false);
  const [sheetsMode, setSheetsMode] = useState<'excel' | 'googlesheets'>('excel');
  const [googleSheetsStatus, setGoogleSheetsStatus] = useState<string>('');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  // Stop functionality
  const [isRequestCancelled, setIsRequestCancelled] = useState(false);
  const pollingIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  useEffect(() => {
    loadChatHistory();
    loadLlmProvider();
  }, []);

  const loadLlmProvider = async () => {
    try {
      const provider = await invoke<string>('get_llm_provider');
      setSelectedLlm(provider as 'gemini' | 'openai');
    } catch (error) {
      console.error('failed to load LLM provider:', error);
    }
  };

  // Check if screen is small
  useEffect(() => {
    const checkScreenSize = () => {
      if (window.innerWidth <= 640) {
        // On small screens, start with sidebar collapsed if chat has messages
        if (chatMessages.length > 0) {
          setSidebarCollapsed(true);
        }
      } else {
        setSidebarCollapsed(false);
      }
    };

    checkScreenSize();
    window.addEventListener('resize', checkScreenSize);
    return () => window.removeEventListener('resize', checkScreenSize);
  }, [chatMessages.length]);

  const toggleSidebar = () => {
    setSidebarCollapsed(!sidebarCollapsed);
  };

  const loadChatHistory = async () => {
    try {
      const history = await invoke<ChatMessage[]>('get_chat_history');
      setChatMessages(history);
    } catch (error) {
      console.error('failed to load chat history:', error);
    }
  };

  const showStatus = (message: string) => {
    setStatus(message);
    setTimeout(() => setStatus('ready'), 3000);
  };

  const handleNewFile = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<string>('create_new_excel');
      setCurrentFile('new workbook');
      showStatus(`excel file created: ${result}`);
    } catch (error) {
      console.error('error creating new file:', error);
      showStatus('failed to create new file');
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenFile = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<string>('open_excel_file');
      setCurrentFile(result);
      showStatus('file opened successfully');
    } catch (error) {
      console.error('error opening file:', error);
      showStatus('failed to open file');
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenGoogleSheets = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<string>('open_google_sheets');
      showStatus(`google sheets opened: ${result}`);
      setTimeout(checkGoogleSheetsAvailability, 2000);
    } catch (error) {
      console.error('error opening google sheets:', error);
      showStatus('failed to open google sheets');
    } finally {
      setIsLoading(false);
    }
  };

  const checkGoogleSheetsAvailability = async () => {
    try {
      const result = await invoke<string>('check_google_sheets_availability');
      setGoogleSheetsStatus(result);
      if (result.includes('available and ready')) {
        showStatus('google sheets with gemini is ready');
      } else {
        showStatus('google sheets availability checked');
      }
    } catch (error) {
      console.error('error checking google sheets:', error);
      setGoogleSheetsStatus('Error checking availability');
    }
  };

  // Removed automatic Google Sheets availability check when switching to Google Sheets mode
  // The user should manually click "check availability" when they want to verify

  // Reconfigure clients when Copilot setting changes
  useEffect(() => {
    const reconfigureClients = async () => {
      if (isGeminiConfigured && geminiApiKey.trim()) {
        console.log(`Reconfiguring Gemini with Copilot ${copilotEnabled ? 'enabled' : 'disabled'}`);
        try {
          await invoke('setup_gemini_client', { 
            apiKey: geminiApiKey, 
            copilotEnabled: copilotEnabled 
          });
          showStatus(`gemini reconfigured with copilot ${copilotEnabled ? 'enabled' : 'disabled'}`);
        } catch (error) {
          console.error('Error reconfiguring Gemini:', error);
        }
      }
      
      if (isOpenAIConfigured && openaiApiKey.trim()) {
        console.log(`Reconfiguring OpenAI with Copilot ${copilotEnabled ? 'enabled' : 'disabled'}`);
        try {
          await invoke('setup_openai_client', { 
            apiKey: openaiApiKey, 
            copilotEnabled: copilotEnabled 
          });
          showStatus(`openai reconfigured with copilot ${copilotEnabled ? 'enabled' : 'disabled'}`);
        } catch (error) {
          console.error('Error reconfiguring OpenAI:', error);
        }
      }
    };

    // Only reconfigure if we have configured clients
    if (isGeminiConfigured || isOpenAIConfigured) {
      reconfigureClients();
    }
  }, [copilotEnabled, isGeminiConfigured, isOpenAIConfigured, geminiApiKey, openaiApiKey]); // Trigger when copilotEnabled changes

  const isLlmConfigured = selectedLlm === 'gemini' ? isGeminiConfigured : isOpenAIConfigured;
  
  const isChatInputDisabled = isLoading || !isLlmConfigured;

  const setupGemini = async () => {
    if (!geminiApiKey.trim()) return;
    
    setIsLoading(true);
    try {
      await invoke('setup_gemini_client', { 
        apiKey: geminiApiKey, 
        copilotEnabled: copilotEnabled 
      });
      setIsGeminiConfigured(true);
      setShowApiKeyInput(false);
      showStatus('gemini configured successfully');
    } catch (error) {
      console.error('error setting up gemini:', error);
      showStatus('failed to configure gemini');
    } finally {
      setIsLoading(false);
    }
  };

  const setupOpenAI = async () => {
    if (!openaiApiKey.trim()) return;
    
    setIsLoading(true);
    try {
      await invoke('setup_openai_client', { 
        apiKey: openaiApiKey, 
        copilotEnabled: copilotEnabled 
      });
      setIsOpenAIConfigured(true);
      setShowApiKeyInput(false);
      showStatus('openai configured successfully');
    } catch (error) {
      console.error('error setting up openai:', error);
      showStatus('failed to configure openai');
    } finally {
      setIsLoading(false);
    }
  };

  const switchLlmProvider = async (provider: 'gemini' | 'openai') => {
    try {
      await invoke('set_llm_provider', { provider });
      setSelectedLlm(provider);
      showStatus(`switched to ${provider}`);
    } catch (error) {
      console.error('error switching LLM provider:', error);
      showStatus('failed to switch LLM provider');
    }
  };

  const stopCurrentRequest = async () => {
    console.log('🛑 User requested to stop current request');
    setIsRequestCancelled(true);
    
    try {
      // Call the backend to stop the LLM request
      await invoke('stop_llm_request');
      console.log('✓ Backend cancellation signal sent');
    } catch (error) {
      console.error('Error stopping request:', error);
    }
    
    // Stop polling interval if active
    if (pollingIntervalRef.current) {
      clearInterval(pollingIntervalRef.current);
      pollingIntervalRef.current = null;
      console.log('✓ Polling interval cleared');
    }
    
    // Abort the request if possible
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
      console.log('✓ Request aborted');
    }
    
    // Reset loading state
    setIsLoading(false);
    
    // Show status
    showStatus('request stopped by user');
    
    // Update the assistant placeholder to show cancellation
    setChatMessages(prev => {
      const updated = [...prev];
      const lastMessage = updated[updated.length - 1];
      if (lastMessage && lastMessage.role === 'model' && lastMessage.content === 'thinking...') {
        lastMessage.content = '❌ Request stopped by user';
        lastMessage.response_details = { has_tool_calls: false, iterations: 0 };
      }
      return updated;
    });
  };

  const sendMessage = async () => {
    if (!currentMessage.trim() || !isLlmConfigured || isLoading) return;

    // Reset cancellation state
    setIsRequestCancelled(false);
    setIsLoading(true);
    
    const userMessageText = currentMessage;
    const pdfsToProcess = [...attachedPdfs];
    setCurrentMessage('');
    setAttachedPdfs([]);

    const newUserMessage: ChatMessage = {
      role: 'user',
      content: userMessageText,
      timestamp: Date.now(),
    };

    const assistantPlaceholder: ChatMessage = {
      role: 'model',
      content: 'thinking...',
      timestamp: Date.now() + 1, // Ensure unique timestamp for placeholder
      tool_calls: [],
      response_details: { has_tool_calls: false, iterations: 0 },
    };

    const chatLengthBeforeLocalUpdate = chatMessages.length;
    console.log(`📤 Sending message: "${userMessageText}"`);

    // Display user message and placeholder
    setChatMessages(prev => [...prev, newUserMessage, assistantPlaceholder]);

    let pollingTimeout: NodeJS.Timeout | null = null;

    try {
      // Create AbortController for request cancellation
      const abortController = new AbortController();
      abortControllerRef.current = abortController;
      
      const processingPromise = pdfsToProcess.length > 0
        ? invoke('chat_with_llm_pdf', { message: userMessageText, pdfFiles: pdfsToProcess })
        : invoke('chat_with_llm', { message: userMessageText });

      const pollForUpdates = setInterval(async () => {
        // Check if request was cancelled
        if (isRequestCancelled) {
          clearInterval(pollForUpdates);
          return;
        }
        try {
          const backendHistory = await invoke<ChatMessage[]>('get_chat_history');
          
          setChatMessages(currentLocalMessages => {
            // Simple check: if backend has more messages than what we expect, update
            const expectedLength = chatLengthBeforeLocalUpdate + 2; // user message + assistant response
            
            if (backendHistory.length >= expectedLength) {
              const lastBackendMessage = backendHistory[backendHistory.length - 1];
              
              // If the last message is from the model and is not "thinking...", the response is complete
              if (lastBackendMessage && 
                  lastBackendMessage.role === 'model' && 
                  lastBackendMessage.content !== 'thinking...') {
                console.log('📝 Response complete detected, updating UI');
                return backendHistory;
              }
            }
            
            // Check if we have a partial response with tool calls to update the placeholder
            if (backendHistory.length > currentLocalMessages.length) {
              const lastBackendMessage = backendHistory[backendHistory.length - 1];
              if (lastBackendMessage && lastBackendMessage.role === 'model') {
                // Update the placeholder with new tool calls or status
                const updatedMessages = [...currentLocalMessages];
                const localPlaceholderIndex = updatedMessages.findIndex(
                  m => m.role === 'model' && m.timestamp === assistantPlaceholder.timestamp
                );
                
                if (localPlaceholderIndex !== -1) {
                  // Replace placeholder with backend version that may have tool calls
                  updatedMessages[localPlaceholderIndex] = lastBackendMessage;
                  return updatedMessages;
                }
              }
            }
            
            return currentLocalMessages;
          });
        } catch (error) {
          console.error('Error polling chat history:', error);
        }
      }, 500); // Reduced polling interval for better responsiveness

      // Store polling interval reference for cancellation
      pollingIntervalRef.current = pollForUpdates;

      // Safety timeout to stop polling after 2 minutes (in case backend doesn't respond properly)
      pollingTimeout = setTimeout(() => {
        console.log('⚠️ Polling timeout reached, forcing stop');
        if (pollingIntervalRef.current) {
          clearInterval(pollingIntervalRef.current);
          pollingIntervalRef.current = null;
        }
        // Force final sync
        loadChatHistory();
      }, 120000); // 2 minutes timeout

      const responseSummary = await processingPromise; // Wait for the main backend operation to complete

      // Check if request was cancelled during processing
      if (isRequestCancelled) {
        console.log('Request was cancelled, stopping here');
        clearInterval(pollForUpdates);
        return;
      }

      console.log('🏁 Backend processing complete, stopping polling');
      clearInterval(pollForUpdates); // Stop polling
      if (pollingTimeout) clearTimeout(pollingTimeout); // Clear timeout
      pollingIntervalRef.current = null;
      
      // Final sync with backend history - this ensures UI is up to date
      console.log('🔄 Final sync with backend history');
      await loadChatHistory();

      showStatus(responseSummary ? `message sent: ${responseSummary}`: 'Message processed');
    } catch (error: any) {
      // Check if error is due to cancellation
      if (isRequestCancelled) {
        console.log('Request was cancelled');
        return;
      }
      
      console.error('Error sending message:', error);
      showStatus(`error: ${error.message || error}`);
      
      // Clear polling if still active
      if (pollingIntervalRef.current) {
        clearInterval(pollingIntervalRef.current);
        pollingIntervalRef.current = null;
      }
      // Clear timeout as well
      if (pollingTimeout) clearTimeout(pollingTimeout);
      
      // Add error message to chat and remove placeholder
      setChatMessages(prev => {
        const updated = prev.filter(msg => msg.timestamp !== assistantPlaceholder.timestamp);
        // Add error message
        updated.push({
          role: 'model',
          content: `❌ Error: ${error.message || error}`,
          timestamp: Date.now(),
          tool_calls: [],
          response_details: { has_tool_calls: false, iterations: 0 },
        });
        return updated;
      });
    } finally {
      // Clean up references
      abortControllerRef.current = null;
      pollingIntervalRef.current = null;
      
      // ALWAYS reset loading state in finally block
      setIsLoading(false);
    }
  };

  const selectPdfFiles = async () => {
    try {
      const selectedFiles = await invoke<string[]>('select_pdf_files');
      if (selectedFiles.length > 0) {
        setAttachedPdfs(prev => [...prev, ...selectedFiles]);
        showStatus(`${selectedFiles.length} pdf(s) attached`);
      }
    } catch (error) {
      console.error('error selecting pdf files:', error);
      showStatus('failed to select pdf files');
    }
  };

  const removePdf = (index: number) => {
    setAttachedPdfs(prev => prev.filter((_, i) => i !== index));
    showStatus('pdf removed');
  };

  const testExcelInteraction = async () => {
    setIsLoading(true);
    try {
      showStatus('testing excel interaction...');
      
      const cellValue = await invoke<string>('excel_read_cell', { cellAddress: 'A1' });
      showStatus(`cell a1: ${cellValue}`);
      
      await invoke<string>('excel_write_cell', { 
        cellAddress: 'B1', 
        value: 'hello from copilot!' 
      });
      
      showStatus('excel interaction test completed');
    } catch (error) {
      console.error('excel interaction test failed:', error);
      showStatus(`excel test failed: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const showLocaleInfo = async () => {
    setIsLoading(true);
    try {
      const localeInfo = await invoke<string>('get_locale_info');
      showStatus(`locale info: ${localeInfo}`);
      
      // Add a message to chat to show locale info
      const userMessage: ChatMessage = {
        role: 'user',
        content: '🌍 System Locale Information Request',
        timestamp: Date.now()
      };
      
      const assistantMessage: ChatMessage = {
        role: 'model',
        content: `📍 **System Locale Information:**\n\n${localeInfo}\n\n*This affects how numbers are formatted when writing to Excel. The system automatically normalizes number formats according to your locale before sending them to Excel.*`,
        timestamp: Date.now()
      };
      
      setChatMessages(prev => [...prev, userMessage, assistantMessage]);
      
    } catch (error) {
      console.error('failed to get locale info:', error);
      showStatus(`locale info failed: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const clearChat = async () => {
    try {
      await invoke('clear_chat_history');
      setChatMessages([]);
      showStatus('chat cleared');
    } catch (error) {
      console.error('error clearing chat:', error);
      showStatus('failed to clear chat');
    }
  };

  const formatTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString();
  };

  return (
    <div className="app">
      {/* Header */}
      <header className="header">
        <div className="header-content">
          <div className="logo">
            <div className="logo-icon">E</div>
            <span className="logo-text">excel copilot</span>
          </div>
          <div className="status">
            <span className="status-text">{status}</span>
            {isLoading && <div className="loading-spinner"></div>}
          </div>
          <div className="llm-indicator">
            <div className={`indicator-dot ${isLlmConfigured ? 'connected' : 'disconnected'}`}></div>
            <span className="indicator-text">
              {isLlmConfigured ? `${selectedLlm} ready` : `configure ${selectedLlm}`}
            </span>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="main">
        {/* Sidebar Toggle Button for Mobile */}
        <button 
          className="sidebar-toggle"
          onClick={toggleSidebar}
          title={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
        >
          {sidebarCollapsed ? '☰' : '×'}
        </button>

        {/* Sidebar */}
        <aside className={`sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
          <section className="file-section">
            <h3 className="section-title">spreadsheet operations</h3>
            <div className="app-mode-selector">
              <div className="mode-option">
                <input
                  type="radio"
                  id="excel-mode"
                  name="sheetsMode"
                  value="excel"
                  checked={sheetsMode === 'excel'}
                  onChange={(e) => setSheetsMode(e.target.value as 'excel' | 'googlesheets')}
                  className="mode-radio"
                />
                <label htmlFor="excel-mode" className="mode-label">
                  <div className="mode-icon">📊</div>
                  <div className="mode-info">
                    <div className="mode-title">Microsoft Excel</div>
                    <div className="mode-subtitle">Local files & automation</div>
                  </div>
                </label>
              </div>
              <div className="mode-option">
                <input
                  type="radio"
                  id="sheets-mode"
                  name="sheetsMode"
                  value="googlesheets"
                  checked={sheetsMode === 'googlesheets'}
                  onChange={(e) => setSheetsMode(e.target.value as 'excel' | 'googlesheets')}
                  className="mode-radio"
                />
                <label htmlFor="sheets-mode" className="mode-label">
                  <div className="mode-icon">🌐</div>
                  <div className="mode-info">
                    <div className="mode-title">Google Sheets</div>
                    <div className="mode-subtitle">Browser-based with Gemini</div>
                  </div>
                </label>
              </div>
            </div>
            
            <div className="button-group">
              {sheetsMode === 'excel' ? (
                <>
                  <button 
                    className="btn btn-primary" 
                    onClick={handleNewFile}
                    disabled={isLoading}
                  >
                    new excel file
                  </button>
                  <button 
                    className="btn btn-secondary" 
                    onClick={handleOpenFile}
                    disabled={isLoading}
                  >
                    open excel file
                  </button>
                  <button 
                    className="btn btn-outline" 
                    onClick={testExcelInteraction}
                    disabled={isLoading || !currentFile}
                  >
                    test excel
                  </button>
                </>
              ) : (
                <>
                  <button 
                    className="btn btn-primary" 
                    onClick={handleOpenGoogleSheets}
                    disabled={isLoading}
                  >
                    open new google sheets
                  </button>
                  <div className="google-sheets-info">
                    <div className="info-header">📋 Setup Required:</div>
                    <div className="info-steps">
                      <div className="info-step">1. Open Google Sheets manually OR click button above</div>
                      <div className="info-step">2. Open the document you want to edit, or create a new one</div>
                      <div className="info-step">3. Ensure Gemini is available in your Google Sheets</div>
                      <div className="info-step">4. Start chatting - I'll interact with Gemini for you</div>
                    </div>
                    <div className="info-header" style={{ marginTop: '12px' }}>💡 Tips for Best Results:</div>
                    <div className="info-steps">
                      <div className="info-step">• Be specific: "Create a table with Name, Age, City columns"</div>
                      <div className="info-step">• One task at a time: "Add this data" then "Format as currency"</div>
                      <div className="info-step">• Use simple language: "Make row 1 bold" vs complex formatting</div>
                    </div>
                    {googleSheetsStatus && (
                      <div className={`availability-status ${googleSheetsStatus.includes('available and ready') ? 'ready' : 'warning'}`}>
                        🔍 Status: {googleSheetsStatus}
                      </div>
                    )}
                    <button 
                      className="btn btn-outline btn-small" 
                      onClick={checkGoogleSheetsAvailability}
                      disabled={isLoading}
                      style={{ marginTop: '12px' }}
                    >
                      check availability
                    </button>
                  </div>
                </>
              )}
              
              <button 
                className="btn btn-outline" 
                onClick={showLocaleInfo}
                disabled={isLoading}
                title="show system locale and number formatting info"
              >
                locale info
              </button>
            </div>
            {currentFile && sheetsMode === 'excel' && (
              <div className="current-file">
                <span className="file-label">current file:</span>
                <span className="file-name">{currentFile}</span>
              </div>
            )}
          </section>

          {sheetsMode === 'excel' && (
            <section className="copilot-section">
              <h3 className="section-title">excel copilot settings</h3>
              <div className="copilot-toggle">
                <label className="toggle-label">
                  <input
                    type="checkbox"
                    checked={copilotEnabled}
                    onChange={(e) => setCopilotEnabled(e.target.checked)}
                    className="toggle-input"
                  />
                  <span className="toggle-slider"></span>
                  <span className="toggle-text">Enable MS Excel Copilot</span>
                </label>
              </div>
              
              {copilotEnabled && (
                <div className="copilot-requirements">
                  <div className="requirements-header">⚠️ Requirements:</div>
                  <ul className="requirements-list">
                    <li>📁 File must be saved in <strong>OneDrive</strong></li>
                    <li>📂 Use <strong>"Open"</strong> (not "New") to select existing OneDrive file</li>
                    <li>📊 Data range needs <strong>≥3 rows, ≥2 columns</strong></li>
                    <li>🏷️ <strong>Headers required</strong> in first row</li>
                  </ul>
                  <div className="requirements-note">
                    💡 Copilot only works with OneDrive documents that have properly structured data with headers.
                  </div>
                </div>
              )}
              
              {copilotEnabled && !currentFile.includes('OneDrive') && currentFile && (
                <div className="copilot-warning">
                  ⚠️ Current file may not be in OneDrive. Copilot features might not work.
                </div>
              )}
            </section>
          )}

          {sheetsMode === 'googlesheets' && (
            <section className="google-sheets-section">
              <h3 className="section-title">google sheets requirements</h3>
              <div className="google-sheets-details">
                <div className="requirement-item">
                  <span className="requirement-icon">🔑</span>
                  <div className="requirement-content">
                    <div className="requirement-title">Gemini Access Required</div>
                    <div className="requirement-text">You need access to Gemini in Google Sheets. Look for the "Ask Gemini" button in your Google Sheets interface.</div>
                  </div>
                </div>
                <div className="requirement-item">
                  <span className="requirement-icon">🌐</span>
                  <div className="requirement-content">
                    <div className="requirement-title">Browser-Based</div>
                    <div className="requirement-text">All operations are performed through Google Sheets' built-in Gemini interface via browser automation.</div>
                  </div>
                </div>
                <div className="requirement-item">
                  <span className="requirement-icon">🤖</span>
                  <div className="requirement-content">
                    <div className="requirement-title">AI-Powered Automation</div>
                    <div className="requirement-text">I'll send requests to Google Sheets Gemini and automatically apply the responses for you.</div>
                  </div>
                </div>
              </div>
            </section>
          )}
        </aside>

        {/* Chat Area */}
        <section className={`chat-section ${sidebarCollapsed ? 'expanded' : ''}`}>
          <div className="chat-header">
            <h2 className="chat-title">
              chat with {selectedLlm} - {sheetsMode === 'excel' ? 'excel mode' : 'google sheets mode'}
            </h2>
            <div className="chat-controls">
              <button 
                className="btn-icon"
                onClick={() => setShowApiKeyInput(!showApiKeyInput)}
                title="configure api key"
              >
                ⚙
              </button>
              <button 
                className="btn-icon"
                onClick={clearChat}
                disabled={chatMessages.length === 0}
                title="clear chat"
              >
                ×
              </button>
            </div>
          </div>

          {showApiKeyInput && (
            <div className="api-key-input">
              <div className="llm-selector">
                <div className="llm-option">
                  <input
                    type="radio"
                    id="llm-gemini"
                    name="llmProvider"
                    value="gemini"
                    checked={selectedLlm === 'gemini'}
                    onChange={(e) => switchLlmProvider(e.target.value as 'gemini' | 'openai')}
                    className="llm-radio"
                  />
                  <label htmlFor="llm-gemini" className="llm-label">
                    <span className="llm-icon">🤖</span>
                    <span className="llm-name">Gemini</span>
                    <span className="llm-status">{isGeminiConfigured ? '✅' : '❌'}</span>
                  </label>
                </div>
                <div className="llm-option">
                  <input
                    type="radio"
                    id="llm-openai"
                    name="llmProvider"
                    value="openai"
                    checked={selectedLlm === 'openai'}
                    onChange={(e) => switchLlmProvider(e.target.value as 'gemini' | 'openai')}
                    className="llm-radio"
                  />
                  <label htmlFor="llm-openai" className="llm-label">
                    <span className="llm-icon">🧠</span>
                    <span className="llm-name">OpenAI GPT-4o-mini</span>
                    <span className="llm-status">{isOpenAIConfigured ? '✅' : '❌'}</span>
                  </label>
                </div>
              </div>
              
              {selectedLlm === 'gemini' ? (
                <div className="api-input-row">
                  <input
                    type="password"
                    placeholder="enter gemini api key"
                    value={geminiApiKey}
                    onChange={(e) => setGeminiApiKey(e.target.value)}
                    onKeyPress={(e) => e.key === 'Enter' && setupGemini()}
                    className="api-input"
                  />
                  <button onClick={setupGemini} disabled={isLoading} className="btn btn-primary">
                    save
                  </button>
                </div>
              ) : (
                <div className="api-input-row">
                  <input
                    type="password"
                    placeholder="enter openai api key"
                    value={openaiApiKey}
                    onChange={(e) => setOpenaiApiKey(e.target.value)}
                    onKeyPress={(e) => e.key === 'Enter' && setupOpenAI()}
                    className="api-input"
                  />
                  <button onClick={setupOpenAI} disabled={isLoading} className="btn btn-primary">
                    save
                  </button>
                </div>
              )}
            </div>
          )}

          <div className="chat-messages">
            {chatMessages.length === 0 ? (
              <div className="empty-chat">
                <div className="welcome-content">
                  <h3>welcome to excel copilot</h3>
                  <p>connect with {selectedLlm} to start analyzing your {sheetsMode === 'excel' ? 'excel' : 'google sheets'} data.</p>
                  <ul className="feature-list">
                    <li>ask questions about your data</li>
                    <li>request summaries and insights</li>
                    <li>get help with formulas</li>
                    <li>auto-generate content</li>
                    {sheetsMode === 'googlesheets' && <li>interact via google sheets gemini</li>}
                  </ul>
                  <div className="example-queries">
                    {sheetsMode === 'excel' ? (
                      <>
                        <div className="query-example">"what's the sum of column a?"</div>
                        <div className="query-example">"create a summary of this data"</div>
                        <div className="query-example">"add a formula to calculate average"</div>
                      </>
                    ) : (
                      <>
                        <div className="query-example">"Add this data to the sheet: Name, Age, City"</div>
                        <div className="query-example">"Create a bar chart from columns A to C"</div>
                        <div className="query-example">"Format column B as currency and make row 1 bold"</div>
                      </>
                    )}
                  </div>
                </div>
              </div>
            ) : (
              chatMessages.map((msg, index) => (
                <div key={index} className={`message ${msg.role}-message`}>
                  <div className="message-content">
                    {msg.content}
                  </div>
                  
                  {msg.tool_calls && msg.tool_calls.length > 0 && (
                    <div className="tool-calls">
                      <div className="tool-calls-header">
                        functions called ({msg.tool_calls.length})
                      </div>
                      {msg.tool_calls.map((toolCall, tcIndex) => (
                        <div key={tcIndex} className="tool-call">
                          <div className="tool-call-name">
                            {toolCall.function_name}
                          </div>
                          <div className="tool-call-args">
                            <strong>arguments:</strong>
                            <pre>{JSON.stringify(toolCall.arguments, null, 2)}</pre>
                          </div>
                          <div className="tool-call-result">
                            <strong>result:</strong>
                            <div className="result-content">{toolCall.result}</div>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                  
                  {msg.role === 'model' && msg.response_details && (
                    <div className="response-details">
                      {msg.response_details.has_tool_calls && (
                        <span className="detail-badge">
                          used {msg.response_details.iterations} iteration(s)
                        </span>
                      )}
                      <span className="detail-badge">
                        llm response
                      </span>
                    </div>
                  )}
                  
                  <div className="message-time">
                    {formatTime(msg.timestamp)}
                  </div>
                </div>
              ))
            )}
          </div>

            <div className="chat-input">
            {!isLlmConfigured && (
              <div className="llm-setup-message">
                <div className="setup-message-content">
                  <span className="setup-icon">🔑</span>
                  <div className="setup-text">
                    <strong>Configure {selectedLlm} API first</strong>
                    <span>Click the settings button (⚙) above to enter your {selectedLlm} API key</span>
                  </div>
                </div>
              </div>
            )}
            
            {isLlmConfigured && attachedPdfs.length > 0 && (
                <div className="pdf-attachments">
                  <div className="attachments-header">
                    attached pdf files ({attachedPdfs.length})
                  </div>
                  <div className="attachments-list">
                    {attachedPdfs.map((pdf, index) => (
                      <div key={index} className="attachment-item">
                        <span className="attachment-name">{pdf}</span>
                        <button 
                          className="remove-attachment"
                          onClick={() => removePdf(index)}
                          title="remove this pdf"
                        >
                          ×
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              
              <div className="input-controls">
                <button 
                  className="attach-btn"
                  onClick={selectPdfFiles}
                disabled={isLoading || !isLlmConfigured}
                title={!isLlmConfigured ? `Configure ${selectedLlm} first` : "attach pdf files"}
                >
                  📎
                </button>
                <input
                  type="text"
                                  placeholder={
                  !isLlmConfigured 
                    ? `Please configure your ${selectedLlm} API key first...` 
                    : `ask me anything about your ${sheetsMode === 'excel' ? 'excel' : 'google sheets'} data...`
                }
                  value={currentMessage}
                  onChange={(e) => setCurrentMessage(e.target.value)}
                  onKeyPress={(e) => e.key === 'Enter' && sendMessage()}
                  disabled={isChatInputDisabled}
                  className="message-input"
                />
                {isLoading ? (
                  <button 
                    className="stop-btn"
                    onClick={stopCurrentRequest}
                    title="Stop current request"
                  >
                    ⏹️
                  </button>
                ) : (
                  <button 
                    className="send-btn"
                    onClick={sendMessage}
                    disabled={isChatInputDisabled || !currentMessage.trim()}
                    title={!isLlmConfigured ? `Configure ${selectedLlm} first` : "Send message"}
                  >
                    →
                  </button>
                )}
              </div>
            </div>
        </section>
      </main>
    </div>
  );
}

export default App;