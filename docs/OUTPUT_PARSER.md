
# Output Parser Documentation

Extract structured data from UI trees using JavaScript.

## Basic Usage

```yaml
output_parser:
  ui_tree_source_step_id: "capture_tree_step"  # Optional
  javascript_code: |
    const results = [];
    // Process the 'tree' variable
    // Return array of objects
    return results;
```

## Essential Patterns

### Find Elements by Role
```javascript
const results = [];

function findElementsRecursively(element) {
    if (element.attributes?.role === 'Button') {
        results.push({
            name: element.attributes.name || '',
            enabled: element.attributes.is_enabled !== false
        });
    }
    
    element.children?.forEach(child => findElementsRecursively(child));
}

findElementsRecursively(tree);
return results;
```

### Find Elements with Children
```javascript
const results = [];

function findElementsRecursively(element) {
    if (element.attributes?.role === 'Group') {
        const children = element.children || [];
        const hasImage = children.some(c => c.attributes?.role === 'Image');
        const hasText = children.some(c => c.attributes?.role === 'Text');
        
        if (hasImage && hasText) {
            const textElements = children.filter(c => 
                c.attributes?.role === 'Text' && c.attributes?.name
            );
            
            results.push({
                groupId: element.attributes.id || '',
                texts: textElements.map(t => t.attributes.name)
            });
        }
    }
    
    element.children?.forEach(child => findElementsRecursively(child));
}

findElementsRecursively(tree);
return results;
```

### Handle "No Data" Cases
```javascript
// Check for "no data" messages first
function hasNoData(element) {
    if (element.attributes?.name?.toLowerCase().includes('no results')) return true;
    return element.children?.some(hasNoData) || false;
}

if (hasNoData(tree)) return [];

// Normal extraction...
const results = [];
// ... extraction logic
return results;
```

## Real Example (Insurance Quotes)

```javascript
// Return empty if ineligible
function checkIneligible(element) {
    const text = element.attributes?.name?.toLowerCase() || '';
    if (text.includes('client is ineligible')) return true;
    return element.children?.some(checkIneligible) || false;
}

if (checkIneligible(tree)) return [];

// Extract quotes
const results = [];

function findQuotes(element) {
    if (element.attributes?.role === 'Group') {
        const children = element.children || [];
        const hasImage = children.some(c => c.attributes?.role === 'Image');
        const hasText = children.some(c => c.attributes?.role === 'Text');
        
        if (hasImage && hasText) {
            const texts = children
                .filter(c => c.attributes?.role === 'Text' && c.attributes?.name)
                .map(c => c.attributes.name);
            
            let carrier = '', price = '';
            for (const text of texts) {
                if (text.includes(':')) carrier = text;
                if (text.startsWith('$')) price = text;
            }
            
            if (carrier && price) {
                results.push({ carrier, price });
            }
        }
    }
    
    element.children?.forEach(findQuotes);
}

findQuotes(tree);
return results;
```

## Tips

- Always use `element.children?.forEach()` for traversal
- Use `element.attributes?.property` to avoid errors
- Return `[]` for "no data" scenarios
- Filter noise/error messages early
- Extract meaningful data only

