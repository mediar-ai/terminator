import React from 'react';

interface FieldMapping {
  name: string;
  hotkey: string;
  value: string;
}

interface FieldMappingPanelProps {
  fieldMappings: FieldMapping[];
  updateFieldMapping: (index: number, field: Partial<FieldMapping>) => void;
}

const FieldMappingPanel: React.FC<FieldMappingPanelProps> = ({ 
  fieldMappings, 
  updateFieldMapping 
}) => {
  // Add local state to prevent direct modifications to parent state
  const handleHotkeyChange = (index: number, value: string) => {
    // Validate hotkey before updating
    const trimmedValue = value.trim();
    if (trimmedValue) {
      updateFieldMapping(index, { hotkey: trimmedValue });
    }
  };
  
  return (
    <div className="field-mapping-panel">
      <h2>Field Mappings</h2>
      <div className="field-mappings">
        {fieldMappings.map((mapping, index) => (
          <div className="field-mapping" key={index}>
            <div className="field-name">{mapping.name}</div>
            <div className="field-hotkey">
              <input
                type="text"
                placeholder="Hotkey (e.g., Tab, Ctrl+A)"
                value={mapping.hotkey}
                onChange={(e) => handleHotkeyChange(index, e.target.value)}
                // Add onBlur to ensure value is properly committed
                onBlur={(e) => handleHotkeyChange(index, e.target.value)}
              />
            </div>
            <div className="field-preview">
              <input
                type="text"
                placeholder="Value preview"
                value={mapping.value}
                readOnly
              />
            </div>
          </div>
        ))}
      </div>
      <div className="help-text">
        <p>Configure the hotkeys for each field. When the shortcut is triggered, 
        the clipboard content will be parsed and the app will press the specified 
        hotkey for each field before typing the value.</p>
        <p>Common hotkeys: Tab (move to next field), Alt+key (access menu items), 
        Ctrl+A (select all), etc.</p>
      </div>
    </div>
  );
};

export default FieldMappingPanel;