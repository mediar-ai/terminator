const { ipcRenderer } = require('electron');

// Create IPC proxy
window.electronAPI = {
    addEmployee: (employeeData) => ipcRenderer.invoke('add-employee', employeeData),
    getEmployees: () => ipcRenderer.invoke('get-employees'),
    minimizeWindow: () => ipcRenderer.send('minimize-window'),
    maximizeWindow: () => ipcRenderer.send('maximize-window'),
    closeWindow: () => ipcRenderer.send('close-window')
};

document.addEventListener('DOMContentLoaded', () => {
    // Window control buttons
    document.getElementById('minimize-btn').addEventListener('click', () => {
        window.electronAPI.minimizeWindow();
    });
    
    document.getElementById('maximize-btn').addEventListener('click', () => {
        window.electronAPI.maximizeWindow();
    });
    
    document.getElementById('close-btn').addEventListener('click', () => {
        window.electronAPI.closeWindow();
    });

    // Tab switching functionality
    const navItems = document.querySelectorAll('.nav-item');
    const contentSections = document.querySelectorAll('.content-section');

    navItems.forEach(item => {
        item.addEventListener('click', () => {
            // Remove active state from all tabs
            navItems.forEach(t => t.classList.remove('active'));
            contentSections.forEach(content => content.classList.remove('active'));

            // Add active state to clicked tab and corresponding content
            item.classList.add('active');
            const targetContent = document.getElementById(item.dataset.tab + '-employee');
            if (targetContent) {
                targetContent.classList.add('active');

                // Load employees when view tab is clicked
                if (item.dataset.tab === 'view') {
                    loadEmployees();
                }
            } else {
                console.error('Could not find target content:', item.dataset.tab + '-employee');
            }
        });
    });

    const form = document.getElementById('onboardingForm');
    const employeesContainer = document.getElementById('employees-container');

    // File upload info display
    const resumeInput = document.getElementById('resume');
    const resumeInfo = document.getElementById('resume-info');

    resumeInput.addEventListener('change', (e) => {
        updateFileInfo(e.target, resumeInfo);
    });

    function updateFileInfo(input, infoElement) {
        if (input.files && input.files[0]) {
            const file = input.files[0];
            const fileSize = (file.size / 1024 / 1024).toFixed(2); // in MB
            
            if (fileSize > 5) {
                alert('File size should not exceed 5MB');
                input.value = '';
                infoElement.textContent = 'No file selected';
            } else {
                infoElement.textContent = `${file.name} (${fileSize} MB)`;
            }
        } else {
            infoElement.textContent = 'No file selected';
        }
    }

    // Add ARIA validation states
    form.addEventListener('input', (e) => {
        const input = e.target;
        if (input.value && input.hasAttribute('required')) {
            input.setAttribute('aria-invalid', 'false');
        } else if (!input.value && input.hasAttribute('required')) {
            input.setAttribute('aria-invalid', 'true');
        }
    });

    // Handle form submission
    form.addEventListener('submit', (e) => {
        e.preventDefault();
        
        // Validate form
        const allInputs = form.querySelectorAll('input[required], select[required]');
        let isValid = true;
        
        allInputs.forEach(input => {
            if (!input.value) {
                input.setAttribute('aria-invalid', 'true');
                isValid = false;
            }
        });

        if (isValid) {
            // Create employee object
            const employee = {
                fullName: document.getElementById('fullName').value,
                email: document.getElementById('email').value,
                phone: document.getElementById('phone').value,
                department: document.getElementById('department').value,
                position: document.getElementById('position').value,
                startDate: document.getElementById('startDate').value,
                resumePath: resumeInput.files[0] ? resumeInput.files[0].name : null
            };

            // Add employee to database
            window.electronAPI.addEmployee(employee)
                .then(() => {
                    // Show success message with custom notification
                    showNotification('Success', 'Employee added successfully!', 'success');
                    
                    // Reset form
                    form.reset();
                    resumeInfo.textContent = 'No file selected';

                    // Switch to view employees tab
                    const viewTab = document.querySelector('.nav-item[data-tab="view"]');
                    if (viewTab) {
                        viewTab.click();
                    }
                })
                .catch(err => {
                    console.error('Error adding employee:', err);
                    showNotification('Error', 'Error adding employee. Please try again.', 'error');
                });
        } else {
            showNotification('Warning', 'Please fill in all required fields!', 'warning');
        }
    });

    // Search and filter functionality
    const searchInput = document.getElementById('employee-search');
    const departmentFilter = document.getElementById('department-filter');
    
    if (searchInput) {
        searchInput.addEventListener('input', () => {
            loadEmployees(searchInput.value, departmentFilter.value);
        });
    }
    
    if (departmentFilter) {
        departmentFilter.addEventListener('change', () => {
            loadEmployees(searchInput.value, departmentFilter.value);
        });
    }

    // Load employees function
    const loadEmployees = (searchTerm = '', departmentFilter = '') => {
        // Check if the container exists
        if (!employeesContainer) {
            console.error('Employee container not found!');
            return;
        }
        
        window.electronAPI.getEmployees()
            .then(employees => {
                // Filter employees if search term or department filter is provided
                let filteredEmployees = employees;
                
                if (searchTerm) {
                    const searchLower = searchTerm.toLowerCase();
                    filteredEmployees = filteredEmployees.filter(emp => 
                        emp.fullName.toLowerCase().includes(searchLower) || 
                        emp.email.toLowerCase().includes(searchLower) ||
                        emp.position.toLowerCase().includes(searchLower)
                    );
                }
                
                if (departmentFilter) {
                    filteredEmployees = filteredEmployees.filter(emp => 
                        emp.department === departmentFilter
                    );
                }
                
                if (!filteredEmployees || filteredEmployees.length === 0) {
                    employeesContainer.innerHTML = '<div class="no-data">No employees found.</div>';
                    return;
                }

                // Clear any existing content
                employeesContainer.innerHTML = '';
                
                // Add each employee
                filteredEmployees.forEach(employee => {
                    const item = document.createElement('div');
                    item.className = 'employee-item';
                    
                    // Format date properly
                    let formattedDate = 'N/A';
                    try {
                        formattedDate = new Date(employee.startDate).toLocaleDateString();
                    } catch (e) {
                        console.warn('Invalid date format:', employee.startDate);
                    }
                    
                    item.innerHTML = `
                        <span>${employee.fullName}</span>
                        <span>${employee.email}</span>
                        <span>${employee.department}</span>
                        <span>${employee.position}</span>
                        <span>${formattedDate}</span>
                    `;
                    employeesContainer.appendChild(item);
                });
            })
            .catch(err => {
                console.error('Error loading employees:', err);
                employeesContainer.innerHTML = '<div class="error-message">Error loading employees. Please try again.</div>';
            });
    };

    // Custom notification function
    function showNotification(title, message, type = 'info') {
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        
        // Add notification content
        notification.innerHTML = `
            <div class="notification-icon">
                <i class="fas ${type === 'success' ? 'fa-check-circle' : type === 'error' ? 'fa-times-circle' : 'fa-exclamation-circle'}"></i>
            </div>
            <div class="notification-content">
                <h4>${title}</h4>
                <p>${message}</p>
            </div>
            <button class="notification-close"><i class="fas fa-times"></i></button>
        `;
        
        // Add to document
        document.body.appendChild(notification);
        
        // Add close button functionality
        const closeBtn = notification.querySelector('.notification-close');
        closeBtn.addEventListener('click', () => {
            notification.classList.add('notification-hiding');
            setTimeout(() => {
                document.body.removeChild(notification);
            }, 300);
        });
        
        // Auto-remove after 5 seconds
        setTimeout(() => {
            if (document.body.contains(notification)) {
                notification.classList.add('notification-hiding');
                setTimeout(() => {
                    if (document.body.contains(notification)) {
                        document.body.removeChild(notification);
                    }
                }, 300);
            }
        }, 5000);
        
        // Animate in
        setTimeout(() => {
            notification.classList.add('notification-show');
        }, 10);
    }

    // Add keyboard navigation
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab') {
            const currentFocus = document.activeElement;
            const nextFocus = currentFocus.tabIndex === -1 ? 
                document.querySelector('[tabindex="0"]') : 
                document.querySelector(`[tabindex="${currentFocus.tabIndex + 1}"]`);
            
            if (nextFocus) {
                nextFocus.focus();
            }
        }
    });

    // Initial load when app starts
    loadEmployees();
    
    // Add notification styles
    const style = document.createElement('style');
    style.textContent = `
        .notification {
            position: fixed;
            top: 20px;
            right: 20px;
            display: flex;
            align-items: center;
            background-color: white;
            border-radius: 8px;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
            padding: 16px;
            width: 320px;
            z-index: 1000;
            transform: translateX(400px);
            opacity: 0;
            transition: transform 0.3s, opacity 0.3s;
        }
        
        .notification-show {
            transform: translateX(0);
            opacity: 1;
        }
        
        .notification-hiding {
            transform: translateX(400px);
            opacity: 0;
        }
        
        .notification-icon {
            margin-right: 12px;
            font-size: 24px;
        }
        
        .notification-success .notification-icon {
            color: #2ecc71;
        }
        
        .notification-error .notification-icon {
            color: #e74c3c;
        }
        
        .notification-warning .notification-icon {
            color: #f39c12;
        }
        
        .notification-content {
            flex: 1;
        }
        
        .notification-content h4 {
            margin: 0 0 4px 0;
            font-size: 16px;
        }
        
        .notification-content p {
            margin: 0;
            color: #7f8c8d;
            font-size: 14px;
        }
        
        .notification-close {
            background: none;
            border: none;
            color: #bdc3c7;
            cursor: pointer;
            font-size: 14px;
            padding: 4px;
        }
        
        .notification-close:hover {
            color: #7f8c8d;
        }
    `;
    document.head.appendChild(style);
});
