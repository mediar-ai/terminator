const sqlite3 = require('sqlite3').verbose();
const path = require('path');

// Initialize database
const db = new sqlite3.Database(path.join(__dirname, 'hr_onboarding.db'), (err) => {
    if (err) {
        console.error(err.message);
    }
    console.log('Connected to the HR Onboarding database.');
});

// Create employees table if it doesn't exist
const createTable = () => {
    const sql = `
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
    `;
    
    db.run(sql, (err) => {
        if (err) {
            console.error('Error creating table:', err.message);
        } else {
            console.log('Employees table created successfully.');
        }
    });
};

// Export database functions
module.exports = {
    db,
    createTable,
    addEmployee: (employee, callback) => {
        const sql = `
            INSERT INTO employees (
                fullName, email, phone, department, position, startDate,
                resumePath
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
        `;
        
        db.run(sql, [
            employee.fullName,
            employee.email,
            employee.phone,
            employee.department,
            employee.position,
            employee.startDate,
            employee.resumePath || null
        ], function(err) {
            if (err) {
                console.error('Error adding employee:', err.message);
                callback(err);
            } else {
                callback(null, this.lastID);
            }
        });
    },
    getAllEmployees: (callback) => {
        const sql = 'SELECT * FROM employees ORDER BY createdAt DESC';
        db.all(sql, [], (err, rows) => {
            if (err) {
                console.error('Error getting employees:', err.message);
                callback(err);
            } else {
                callback(null, rows);
            }
        });
    }
};
