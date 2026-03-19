// Global toast notification system.
// Matches the transfer progress toast visual style.

(function (exports) {
  'use strict';

  let toastContainer = null;

  function ensureContainer() {
    if (toastContainer) return;
    toastContainer = document.createElement('div');
    toastContainer.id = 'toast-container';
    document.body.appendChild(toastContainer);
  }

  /**
   * Show a toast notification.
   * @param {Object} opts
   * @param {string} opts.title    — Bold header text
   * @param {string} [opts.body]   — Optional detail text below the title
   * @param {'info'|'success'|'error'|'warn'} [opts.level='info']
   * @param {number} [opts.duration=4000] — Auto-dismiss ms (0 = sticky)
   * @returns {HTMLElement} the toast element (for manual removal)
   */
  function show(opts) {
    ensureContainer();

    const level = opts.level || 'info';
    const duration = opts.duration != null ? opts.duration : 4000;

    const toast = document.createElement('div');
    toast.className = 'conch-toast conch-toast-' + level;

    const icon = level === 'success' ? '\u2713'
      : level === 'error' ? '\u2717'
      : level === 'warn' ? '\u26A0'
      : '\u2139';

    toast.innerHTML = `
      <div class="conch-toast-header">
        <span class="conch-toast-icon">${icon}</span>
        <span class="conch-toast-title">${esc(opts.title)}</span>
        <button class="conch-toast-close">\u2715</button>
      </div>
      ${opts.body ? `<div class="conch-toast-body">${esc(opts.body)}</div>` : ''}
    `;

    toast.querySelector('.conch-toast-close').addEventListener('click', () => dismiss(toast));

    toastContainer.appendChild(toast);
    requestAnimationFrame(() => toast.classList.add('visible'));

    if (duration > 0) {
      toast._timeout = setTimeout(() => dismiss(toast), duration);
    }

    return toast;
  }

  function dismiss(toast) {
    if (!toast || !toast.parentNode) return;
    clearTimeout(toast._timeout);
    toast.classList.remove('visible');
    setTimeout(() => toast.remove(), 300);
  }

  // Convenience methods
  function info(title, body) { return show({ title, body, level: 'info' }); }
  function success(title, body) { return show({ title, body, level: 'success' }); }
  function error(title, body) { return show({ title, body, level: 'error', duration: 6000 }); }
  function warn(title, body) { return show({ title, body, level: 'warn', duration: 5000 }); }

  const esc = window.utils.esc;

  exports.toast = { show, dismiss, info, success, error, warn };
})(window);
