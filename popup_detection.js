// Generic popup detection
const results = {
    found: [],
    summary: ''
};

// Check all elements for popup indicators
document.querySelectorAll('*').forEach(el => {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();

    // Check if visible
    if (el.offsetParent === null) return;

    // Multiple popup detection criteria
    const isPopup =
        // High z-index
        (style.zIndex !== 'auto' && parseInt(style.zIndex) > 1000) ||
        // Role attributes
        (el.getAttribute('role') === 'dialog' || el.getAttribute('role') === 'alertdialog') ||
        // Common popup classes
        (el.className && el.className.toString().match(/modal|popup|dialog|overlay|alert|notification/i)) ||
        // Fixed/absolute with significant size
        ((style.position === 'fixed' || style.position === 'absolute') &&
         rect.width > window.innerWidth * 0.3 && rect.height > window.innerHeight * 0.2);

    if (isPopup) {
        const text = (el.innerText || el.textContent || '').trim().substring(0, 300);
        if (text) {
            results.found.push({
                tag: el.tagName,
                class: el.className || '',
                id: el.id || '',
                zIndex: style.zIndex,
                position: style.position,
                role: el.getAttribute('role'),
                text: text
            });
        }
    }
});

results.summary = `Found ${results.found.length} potential popups`;
console.log(results.summary);
if (results.found.length > 0) {
    console.log('Popup contents:');
    results.found.forEach(p => {
        console.log(`- ${p.tag}.${p.class}: "${p.text.substring(0, 100)}..."`);
    });
}

JSON.stringify(results);