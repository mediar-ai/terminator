import React from 'react';

interface StatusPanelProps {
  logs: string[];
}

const StatusPanel: React.FC<StatusPanelProps> = ({ logs }) => {
  return (
    <div className="status-panel">
      <h2>Activity Log</h2>
      <div className="log-container">
        {logs.length === 0 ? (
          <div className="empty-log">No activity yet</div>
        ) : (
          <pre className="logs">
            {logs.join('\n')}
          </pre>
        )}
      </div>
    </div>
  );
};

export default StatusPanel;