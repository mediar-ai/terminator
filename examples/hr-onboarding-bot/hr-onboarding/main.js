const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const sqlite3 = require('sqlite3').verbose();

// Initialize database
const db = new sqlite3.Database(path.join(__dirname, 'hr_onboarding.db'), (err) => {
    if (err) {
        console.error(err.message);
    }
    console.log('Connected to the HR Onboarding database.');
});

// Create employees table if it doesn't exist
db.run(`
    CREATE TABLE IF NOT EXISTS employees (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        fullName TEXT NOT NULL,
        email TEXT NOT NULL,
        phone TEXT NOT NULL,
        department TEXT NOT NULL,
        position TEXT NOT NULL,
        startDate TEXT NOT NULL,
        resumePath TEXT,
        createdAt TEXT DEFAULT CURRENT_TIMESTAMP
    )
`, (err) => {
    if (err) {
        console.error('Error creating table:', err.message);
    } else {
        console.log('Employees table created successfully.');
    }
});

function createWindow() {
  const win = new BrowserWindow({
    width: 1200,
    height: 800,
    minWidth: 900,
    minHeight: 700,
    frame: false,
    titleBarStyle: 'hidden',
    backgroundColor: '#f8f9fa',
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false
    }
  });

  win.loadFile('index.html');
  
  // Handle window controls
  ipcMain.on('minimize-window', () => {
    win.minimize();
  });

  ipcMain.on('maximize-window', () => {
    if (win.isMaximized()) {
      win.unmaximize();
    } else {
      win.maximize();
    }
  });

  ipcMain.on('close-window', () => {
    win.close();
  });
}

app.whenReady().then(() => {
  createWindow();

  // Load initial employees when app starts
  db.all('SELECT * FROM employees ORDER BY createdAt DESC', [], (err, rows) => {
    if (err) {
      console.error('Error loading initial employees:', err);
    } else {
      console.log('Initial employees loaded:', rows);
    }
  });

  // IPC Handlers
  ipcMain.handle('add-employee', async (event, employeeData) => {
    return new Promise((resolve, reject) => {
      const sql = `
        INSERT INTO employees (
          fullName, email, phone, department, position, startDate,
          resumePath
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
      `;
      
      db.run(sql, [
        employeeData.fullName,
        employeeData.email,
        employeeData.phone,
        employeeData.department,
        employeeData.position,
        employeeData.startDate,
        employeeData.resumePath || null
      ], function(err) {
        if (err) {
          console.error('Error adding employee:', err);
          reject(err);
        } else {
          console.log('Employee added successfully:', this.lastID);
          resolve(this.lastID);
        }
      });
    });
  });

  ipcMain.handle('get-employees', async () => {
    return new Promise((resolve, reject) => {
      db.all('SELECT * FROM employees ORDER BY createdAt DESC', [], (err, rows) => {
        if (err) {
          console.error('Error getting employees:', err);
          reject(err);
        } else {
          console.log('Employees retrieved:', rows);
          resolve(rows);
        }
      });
    });
  });

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// Close database connection when app quits
app.on('will-quit', () => {
  db.close((err) => {
    if (err) {
      console.error('Error closing database:', err.message);
    }
    console.log('Database connection closed.');
  });
});
