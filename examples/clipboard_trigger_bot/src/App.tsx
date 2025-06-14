import { useState, useEffect } from 'react';
import './App.css';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { readText } from '@tauri-apps/api/clipboard';
import FieldMappingPanel from './components/FieldMappingPanel';
import StatusPanel from './components/StatusPanel';

interface FieldMapping {
  name: string;
  hotkey: string;
  value: string;
}

function App() {
  const [status, setStatus] = useState('Idle');
  const [lastTriggered, setLastTriggered] = useState<Date | null>(null);
  const [fieldMappings, setFieldMappings] = useState<FieldMapping[]>([
    { name: 'Field 1', hotkey: 'Tab', value: '' },
    { name: 'Field 2', hotkey: 'Tab', value: '' },
    { name: 'Field 3', hotkey: 'Tab', value: '' },
  ]);
  const [logs, setLogs] = useState<string[]>([]);

  const addLog = (message: string) => {
    const timestamp = new Date().toISOString();
    setLogs(prev => [`[${timestamp}] ${message}`, ...prev.slice(0, 49)]);
  };
  
  const loadMappings = async () => {
      try {
        const savedMappings = await invoke<FieldMapping[]>('get_field_mappings');
        if (savedMappings && savedMappings.length > 0) {
          setFieldMappings(savedMappings);
          addLog('Loaded saved field mappings');
        }
      } catch (error) {
        console.error('Failed to load field mappings:', error);
        addLog(`Failed to load field mappings: ${error}`);
      }
    };

  // Load saved field mappings on startup (runs only once at component mount)
  useEffect(() => {
    loadMappings();
  }, []); // Empty dependency array ensures this runs only once

  // Parse clipboard content
  const parseClipboardContent = async (): Promise<Record<string, string>> => {
    try {
      const text = await readText();
      
      if (!text) {
        throw new Error('Clipboard is empty');
      }

      // Try to parse as JSON first
      try {
        const json = JSON.parse(text);
        if (typeof json === 'object' && json !== null) {
          return json;
        }
      } catch (e) {
        // Not valid JSON, continue with other parsing methods
      }

      // Try to parse as key-value pairs (e.g., "key1: value1\nkey2: value2")
      const keyValuePairs = text.split(/\r?\n/).reduce((acc, line) => {
        const match = line.match(/^([^:]+):\s*(.*)$/);
        if (match) {
          const [, key, value] = match;
          acc[key.trim()] = value.trim();
        }
        return acc;
      }, {} as Record<string, string>);

      if (Object.keys(keyValuePairs).length > 0) {
        return keyValuePairs;
      }

      // If we couldn't parse it as structured data, just split by newlines
      const lines = text.split(/\r?\n/).filter(line => line.trim());
      
      if (lines.length === 0) {
        throw new Error('No content found in clipboard');
      }

      // Map to field names based on line position
      const result: Record<string, string> = {};
      const fieldNames = fieldMappings.map(m => m.name);
      
      for (let i = 0; i < Math.min(fieldNames.length, lines.length); i++) {
        result[fieldNames[i]] = lines[i];
      }

      return result;
    } catch (error) {
      console.error('Error parsing clipboard:', error);
      throw new Error(`Failed to parse clipboard content: ${error}`);
    }
  };

  // Fill form fields using direct input
  const fillFormFields = async (data: Record<string, string>) => {
    setStatus('Filling fields...');
    
    try {
      let successCount = 0;
      
      // Update field values in state
      const newMappings = fieldMappings.map(mapping => ({
        ...mapping,
        value: data[mapping.name] || '',
      }));
      
      setFieldMappings(newMappings);
      
      // Save the updated mappings
      await invoke('update_field_mappings', { mappings: newMappings });
      
      // Fill each field
      for (const mapping of newMappings) {
        if (mapping.hotkey && mapping.value) {
          addLog(`Filling ${mapping.name} with "${mapping.value}" using hotkey "${mapping.hotkey}"`);
          
          const success = await invoke<boolean>('fill_field', { 
            fieldHotkey: mapping.hotkey, 
            value: mapping.value 
          });
          
          if (success) {
            successCount++;
            addLog(`Successfully filled ${mapping.name}`);
          } else {
            addLog(`Failed to fill ${mapping.name}`);
          }
        }
      }
      
      setStatus(`${successCount} fields filled`);
    } catch (error) {
      setStatus('Error filling fields');
      addLog(`Error filling fields: ${error}`);
    }
  };

  useEffect(() => {
    const setupShortcut = async () => {
      try {
        // Register the shortcut
        await invoke('register_shortcut', { 
          shortcut: 'CommandOrControl+Shift+F'
        });
        addLog('Shortcut registered: Ctrl+Shift+F');
        
        // Listen for shortcut triggers
        const unlisten = await listen('shortcut-triggered', async () => {
          setStatus('Triggered');
          setLastTriggered(new Date());
          addLog('Shortcut triggered - Ctrl+Shift+F');

          try {
            const clipboardData = await parseClipboardContent();
            addLog(`Clipboard parsed: ${JSON.stringify(clipboardData)}`);
            
            // Fill the form fields with the parsed data
            await fillFormFields(clipboardData);
          } catch (error) {
            console.error('Error handling clipboard data:', error);
            setStatus('Error');
            addLog(`Error: ${error}`);
          }
        });
        
        return () => {
          unlisten();
          invoke('unregister_shortcut');
        };
      } catch (error) {
        console.error('Failed to register shortcut:', error);
        addLog(`Failed to register shortcut: ${error}`);
      }
    };
    
    setupShortcut();
  }, [fieldMappings]);

  const updateFieldMapping = (index: number, field: Partial<FieldMapping>) => {
    const newMappings = [...fieldMappings];
    newMappings[index] = { ...newMappings[index], ...field };
    setFieldMappings(newMappings);
    
    // Save the updated mappings
    invoke('update_field_mappings', { mappings: newMappings }).catch(error => {
      console.error('Failed to save field mappings:', error);
      addLog(`Failed to save field mappings: ${error}`);
    });
  };

  return (
    <div className="container">
      <h1>ClipboardTriggerBot</h1>
      
      <div className="status-bar">
        <div className="status-message">{status}</div>
        {lastTriggered && (
          <div className="last-triggered">
            Last triggered: {lastTriggered.toLocaleTimeString()}
          </div>
        )}
      </div>

      <div className="panels">
        <FieldMappingPanel 
          fieldMappings={fieldMappings} 
          updateFieldMapping={updateFieldMapping} 
        />
        <StatusPanel logs={logs} />
      </div>

      <div className="instructions">
        <h3>Instructions:</h3>
        <ol>
          <li>Configure field mappings and hotkeys (Tab will move to next field)</li>
          <li>Focus the target application window</li>
          <li>Copy structured text to clipboard</li>
          <li>Press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd> to trigger form filling</li>
        </ol>
      </div>
    </div>
  );
}

export default App;