import React, { useState, useEffect, useRef } from 'react';
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
  const [geminiApiKey, setGeminiApiKey] = useState('');
  const [showApiKeyInput, setShowApiKeyInput] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [status, setStatus] = useState('ready');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const [attachedPdfs, setAttachedPdfs] = useState<string[]>([]);

  useEffect(() => {
    loadChatHistory();
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chatMessages]);

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
      showStatus('excel file created');
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

  const setupGemini = async () => {
    if (!geminiApiKey.trim()) return;
    
    setIsLoading(true);
    try {
      await invoke('setup_gemini_client', { apiKey: geminiApiKey });
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

  const sendMessage = async () => {
    if (!currentMessage.trim() || !isGeminiConfigured || isLoading) return;

    setIsLoading(true);
    const userMessage = currentMessage;
    const pdfsToSend = [...attachedPdfs];
    setCurrentMessage('');
    setAttachedPdfs([]);

    const newUserMessage: ChatMessage = {
      role: 'user',
      content: userMessage,
      timestamp: Date.now()
    };
    setChatMessages(prev => [...prev, newUserMessage]);

    const assistantPlaceholder: ChatMessage = {
      role: 'model',
      content: 'thinking...',
      timestamp: Date.now(),
      tool_calls: [],
      response_details: {
        has_tool_calls: false,
        iterations: 0
      }
    };
    setChatMessages(prev => [...prev, assistantPlaceholder]);

    try {
      const pollForUpdates = setInterval(async () => {
        try {
          const history = await invoke<ChatMessage[]>('get_chat_history');
          setChatMessages(history);
        } catch (error) {
          console.error('error polling chat history:', error);
        }
      }, 200);

      const response = pdfsToSend.length > 0 
        ? await invoke<string>('chat_with_gemini_pdf', { message: userMessage, pdfFiles: pdfsToSend })
        : await invoke<string>('chat_with_gemini', { message: userMessage });
      
      clearInterval(pollForUpdates);
      await loadChatHistory();
      
      showStatus('message sent');
    } catch (error) {
      console.error('error sending message:', error);
      showStatus(`error: ${error}`);
      setChatMessages(prev => prev.slice(0, -1));
    } finally {
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
          <div className="gemini-indicator">
            <div className={`indicator-dot ${isGeminiConfigured ? 'connected' : 'disconnected'}`}></div>
            <span className="indicator-text">
              {isGeminiConfigured ? 'gemini ready' : 'configure gemini'}
            </span>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="main">
        {/* Sidebar */}
        <aside className="sidebar">
          <section className="file-section">
            <h3 className="section-title">file operations</h3>
            <div className="button-group">
              <button 
                className="btn btn-primary" 
                onClick={handleNewFile}
                disabled={isLoading}
              >
                new file
              </button>
              <button 
                className="btn btn-secondary" 
                onClick={handleOpenFile}
                disabled={isLoading}
              >
                open file
              </button>
              <button 
                className="btn btn-outline" 
                onClick={testExcelInteraction}
                disabled={isLoading || !currentFile}
              >
                test excel
              </button>
            </div>
            {currentFile && (
              <div className="current-file">
                <span className="file-label">current file:</span>
                <span className="file-name">{currentFile}</span>
              </div>
            )}
          </section>
        </aside>

        {/* Chat Area */}
        <section className="chat-section">
          <div className="chat-header">
            <h2 className="chat-title">chat with gemini</h2>
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
          )}

          <div className="chat-messages">
            {chatMessages.length === 0 ? (
              <div className="empty-chat">
                <div className="welcome-content">
                  <h3>welcome to excel copilot</h3>
                  <p>connect with gemini to start analyzing your excel data.</p>
                  <ul className="feature-list">
                    <li>ask questions about your data</li>
                    <li>request summaries and insights</li>
                    <li>get help with formulas</li>
                    <li>auto-generate content</li>
                  </ul>
                  <div className="example-queries">
                    <div className="query-example">"what's the sum of column a?"</div>
                    <div className="query-example">"create a summary of this data"</div>
                    <div className="query-example">"add a formula to calculate average"</div>
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
            <div ref={messagesEndRef} />
          </div>

          {isGeminiConfigured && (
            <div className="chat-input">
              {attachedPdfs.length > 0 && (
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
                  disabled={isLoading}
                  title="attach pdf files"
                >
                  📎
                </button>
                <input
                  type="text"
                  placeholder="ask me anything about your excel data..."
                  value={currentMessage}
                  onChange={(e) => setCurrentMessage(e.target.value)}
                  onKeyPress={(e) => e.key === 'Enter' && sendMessage()}
                  disabled={isLoading}
                  className="message-input"
                />
                <button 
                  className="send-btn"
                  onClick={sendMessage}
                  disabled={isLoading || !currentMessage.trim()}
                >
                  {isLoading ? '⏳' : '→'}
                </button>
              </div>
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

export default App;
