// Connections tab — quick connect bar, saved servers, quick actions.

(function (exports) {
  'use strict';

  const { esc, attr } = window.utils;

  /** Render the Connections tab page as a DOM element. */
  function render() {
    const page = document.createElement('div');
    page.className = 'tab-page';

    page.innerHTML = `
      <h1 class="section-title">Connections</h1>

      <!-- Quick connect -->
      <div class="quick-connect-bar">
        <input
          class="quick-connect-input"
          type="text"
          placeholder="user@host or host"
          id="qc-input"
          autocomplete="off"
          autocorrect="off"
          autocapitalize="none"
          spellcheck="false"
        >
        <button class="btn btn-primary" id="qc-connect-btn">Connect</button>
      </div>

      <!-- Saved servers -->
      <div class="card" id="saved-servers-card">
        <div class="card-header">
          <span class="card-title">Saved Servers</span>
          <button class="btn-icon" id="add-server-btn" aria-label="Add server">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 2a1 1 0 0 1 1 1v4h4a1 1 0 1 1 0 2H9v4a1 1 0 1 1-2 0V9H3a1 1 0 0 1 0-2h4V3a1 1 0 0 1 1-1z"/>
            </svg>
          </button>
        </div>
        <div id="servers-list">
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="2" y="3" width="20" height="14" rx="2"/>
              <path d="M8 21h8M12 17v4"/>
            </svg>
            <span class="empty-state-title">No saved servers</span>
            <span class="empty-state-body">Tap + to add your first server</span>
          </div>
        </div>
      </div>

      <!-- Quick actions -->
      <div class="card">
        <div class="card-header">
          <span class="card-title">Quick Actions</span>
        </div>
        <div class="list-item" id="qa-add-host">
          <div class="list-item-icon" style="background:rgba(189,147,249,0.15)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--purple)" stroke-width="1.75">
              <rect x="2" y="3" width="20" height="14" rx="2"/>
              <path d="M8 21h8M12 17v4"/>
            </svg>
          </div>
          <div class="list-item-body">
            <div class="list-item-title">Add Host</div>
            <div class="list-item-subtitle">Configure a new SSH server</div>
          </div>
          <span class="list-item-chevron">
            <svg width="8" height="14" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M1 1l6 6-6 6"/>
            </svg>
          </span>
        </div>
        <div class="list-item" id="qa-sftp">
          <div class="list-item-icon" style="background:rgba(80,250,123,0.15)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--green)" stroke-width="1.75">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
          </div>
          <div class="list-item-body">
            <div class="list-item-title">SFTP Browser</div>
            <div class="list-item-subtitle">Browse remote files</div>
          </div>
          <span class="list-item-chevron">
            <svg width="8" height="14" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M1 1l6 6-6 6"/>
            </svg>
          </span>
        </div>
      </div>
    `;

    // Wire up quick connect
    const qcInput = page.querySelector('#qc-input');
    const qcBtn   = page.querySelector('#qc-connect-btn');

    qcBtn.addEventListener('click', () => handleQuickConnect(qcInput.value.trim()));
    qcInput.addEventListener('keydown', e => {
      if (e.key === 'Enter') handleQuickConnect(qcInput.value.trim());
    });

    // Quick actions (placeholders — wired up when commands are implemented)
    page.querySelector('#qa-add-host').addEventListener('click', () => {
      window.toast.info('Add Host', 'Server management coming soon.');
    });
    page.querySelector('#qa-sftp').addEventListener('click', () => {
      window.toast.info('SFTP Browser', 'Connect to a server first.');
    });
    page.querySelector('#add-server-btn').addEventListener('click', () => {
      window.toast.info('Add Server', 'Server management coming soon.');
    });

    return page;
  }

  function handleQuickConnect(target) {
    if (!target) {
      window.toast.warn('Quick Connect', 'Enter a host or user@host to connect.');
      return;
    }
    // Open terminal and connect
    window.terminalView.connect(target);
  }

  exports.connectionsTab = { render };
})(window);
