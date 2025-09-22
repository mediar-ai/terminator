// Comprehensive login form analysis
const analysis = {
  formFields: {},
  buttonState: {},
  validation: {},
  eventListeners: {},
  windowVariables: {},
  ajaxIndicators: {},
  formElement: null
};

// 1. Find and analyze the domain_user field
const domainUser = document.querySelector('input[name="domain_user"], #domain_user, input[id*="user"], input[placeholder*="user"]');
if (domainUser) {
  analysis.formFields.domainUser = {
    element: domainUser.tagName + (domainUser.id ? '#' + domainUser.id : '') + (domainUser.className ? '.' + domainUser.className.split(' ').join('.') : ''),
    value: domainUser.value,
    type: domainUser.type,
    disabled: domainUser.disabled,
    readOnly: domainUser.readOnly,
    required: domainUser.required,
    pattern: domainUser.pattern,
    minLength: domainUser.minLength,
    maxLength: domainUser.maxLength,
    placeholder: domainUser.placeholder,
    dataAttributes: Object.fromEntries([...domainUser.attributes].filter(attr => attr.name.startsWith('data-')).map(attr => [attr.name, attr.value])),
    classList: [...domainUser.classList],
    validationMessage: domainUser.validationMessage,
    validity: {
      valid: domainUser.validity.valid,
      badInput: domainUser.validity.badInput,
      customError: domainUser.validity.customError,
      patternMismatch: domainUser.validity.patternMismatch,
      rangeOverflow: domainUser.validity.rangeOverflow,
      rangeUnderflow: domainUser.validity.rangeUnderflow,
      stepMismatch: domainUser.validity.stepMismatch,
      tooLong: domainUser.validity.tooLong,
      tooShort: domainUser.validity.tooShort,
      typeMismatch: domainUser.validity.typeMismatch,
      valueMissing: domainUser.validity.valueMissing
    }
  };
}

// 2. Find and analyze the domain_password field
const domainPassword = document.querySelector('input[name="domain_password"], #domain_password, input[id*="password"], input[type="password"]');
if (domainPassword) {
  analysis.formFields.domainPassword = {
    element: domainPassword.tagName + (domainPassword.id ? '#' + domainPassword.id : '') + (domainPassword.className ? '.' + domainPassword.className.split(' ').join('.') : ''),
    value: domainPassword.value ? '[REDACTED - ' + domainPassword.value.length + ' chars]' : 'empty',
    type: domainPassword.type,
    disabled: domainPassword.disabled,
    readOnly: domainPassword.readOnly,
    required: domainPassword.required,
    pattern: domainPassword.pattern,
    minLength: domainPassword.minLength,
    maxLength: domainPassword.maxLength,
    placeholder: domainPassword.placeholder,
    dataAttributes: Object.fromEntries([...domainPassword.attributes].filter(attr => attr.name.startsWith('data-')).map(attr => [attr.name, attr.value])),
    classList: [...domainPassword.classList],
    validationMessage: domainPassword.validationMessage,
    validity: {
      valid: domainPassword.validity.valid,
      badInput: domainPassword.validity.badInput,
      customError: domainPassword.validity.customError,
      patternMismatch: domainPassword.validity.patternMismatch,
      rangeOverflow: domainPassword.validity.rangeOverflow,
      rangeUnderflow: domainPassword.validity.rangeUnderflow,
      stepMismatch: domainPassword.validity.stepMismatch,
      tooLong: domainPassword.validity.tooLong,
      tooShort: domainPassword.validity.tooShort,
      typeMismatch: domainPassword.validity.typeMismatch,
      valueMissing: domainPassword.validity.valueMissing
    }
  };
}

// 3. Find and analyze the logon_sbo_btn button
const logonBtn = document.querySelector('input[name="logon_sbo_btn"], #logon_sbo_btn, button[id*="logon"], input[type="submit"]');
if (logonBtn) {
  analysis.buttonState = {
    element: logonBtn.tagName + (logonBtn.id ? '#' + logonBtn.id : '') + (logonBtn.className ? '.' + logonBtn.className.split(' ').join('.') : ''),
    type: logonBtn.type,
    value: logonBtn.value,
    disabled: logonBtn.disabled,
    onclick: logonBtn.onclick ? logonBtn.onclick.toString() : null,
    dataAttributes: Object.fromEntries([...logonBtn.attributes].filter(attr => attr.name.startsWith('data-')).map(attr => [attr.name, attr.value])),
    classList: [...logonBtn.classList],
    formAttribute: logonBtn.form ? logonBtn.form.id || 'form found' : 'no form'
  };
}

// 4. Find the form element and analyze it
let formElement = null;
if (domainUser || domainPassword) {
  formElement = (domainUser || domainPassword).closest('form');
}
if (!formElement) {
  formElement = document.querySelector('form');
}

if (formElement) {
  analysis.formElement = {
    element: formElement.tagName + (formElement.id ? '#' + formElement.id : '') + (formElement.className ? '.' + formElement.className.split(' ').join('.') : ''),
    action: formElement.action,
    method: formElement.method,
    enctype: formElement.enctype,
    target: formElement.target,
    noValidate: formElement.noValidate,
    dataAttributes: Object.fromEntries([...formElement.attributes].filter(attr => attr.name.startsWith('data-')).map(attr => [attr.name, attr.value])),
    classList: [...formElement.classList],
    onsubmit: formElement.onsubmit ? formElement.onsubmit.toString() : null,
    checkValidity: formElement.checkValidity ? formElement.checkValidity() : 'not available'
  };
}

// 5. Check for event listeners (this is limited but we can try)
try {
  const elements = [domainUser, domainPassword, logonBtn, formElement].filter(Boolean);
  analysis.eventListeners = {};
  elements.forEach((el, index) => {
    const elementName = ['domainUser', 'domainPassword', 'logonBtn', 'formElement'][index];
    if (el) {
      const events = ['click', 'submit', 'change', 'input', 'keydown', 'keyup', 'focus', 'blur'];
      analysis.eventListeners[elementName] = {};
      events.forEach(eventType => {
        const handlerProp = 'on' + eventType;
        if (el[handlerProp]) {
          analysis.eventListeners[elementName][eventType] = el[handlerProp].toString();
        }
      });
    }
  });
} catch (e) {
  analysis.eventListeners.error = e.message;
}

// 6. Check for relevant window variables
try {
  analysis.windowVariables = {
    jQuery: typeof window.jQuery !== 'undefined' ? 'available' : 'not available',
    $: typeof window.$ !== 'undefined' ? 'available' : 'not available',
    angular: typeof window.angular !== 'undefined' ? 'available' : 'not available',
    React: typeof window.React !== 'undefined' ? 'available' : 'not available',
    Vue: typeof window.Vue !== 'undefined' ? 'available' : 'not available'
  };

  const sapVariables = [];
  for (let prop in window) {
    if (prop.toLowerCase().includes('sap') || prop.toLowerCase().includes('login') || prop.toLowerCase().includes('auth')) {
      sapVariables.push(prop);
    }
  }
  analysis.windowVariables.sapRelated = sapVariables;
} catch (e) {
  analysis.windowVariables.error = e.message;
}

// 7. Check for AJAX/fetch indicators
try {
  analysis.ajaxIndicators = {
    xhrActive: XMLHttpRequest.prototype.open ? 'XMLHttpRequest available' : 'XMLHttpRequest not available',
    fetchActive: typeof fetch !== 'undefined' ? 'fetch available' : 'fetch not available',
    loadingIndicators: []
  };

  const loadingSelectors = [
    '[class*="loading"]',
    '[class*="spinner"]',
    '[class*="processing"]',
    '[id*="loading"]',
    '[id*="spinner"]'
  ];

  loadingSelectors.forEach(selector => {
    const elements = document.querySelectorAll(selector);
    if (elements.length > 0) {
      analysis.ajaxIndicators.loadingIndicators.push({
        selector: selector,
        count: elements.length,
        visible: [...elements].map(el => el.offsetWidth > 0 && el.offsetHeight > 0)
      });
    }
  });
} catch (e) {
  analysis.ajaxIndicators.error = e.message;
}

// 8. Additional form validation checks
try {
  analysis.validation.formValidation = {
    html5Validation: formElement ? formElement.checkValidity() : 'no form found',
    requiredFields: [],
    invalidFields: []
  };

  if (formElement) {
    const allInputs = formElement.querySelectorAll('input, select, textarea');
    allInputs.forEach(input => {
      if (input.required) {
        analysis.validation.formValidation.requiredFields.push({
          name: input.name || input.id || 'unnamed',
          type: input.type,
          value: input.type === 'password' ? '[REDACTED]' : input.value,
          valid: input.validity.valid
        });
      }
      if (!input.validity.valid) {
        analysis.validation.formValidation.invalidFields.push({
          name: input.name || input.id || 'unnamed',
          type: input.type,
          validationMessage: input.validationMessage,
          validity: input.validity
        });
      }
    });
  }
} catch (e) {
  analysis.validation.error = e.message;
}

return JSON.stringify(analysis, null, 2);