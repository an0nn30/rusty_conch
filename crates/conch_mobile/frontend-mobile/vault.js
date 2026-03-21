// Vault tab — active sessions, tunnels, transfers.

(function (exports) {
  'use strict';

  const CHEVRON = `<svg width="8" height="14" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M1 1l6 6-6 6"/></svg>`;

  function emptyCard(title, iconSvg, emptyTitle, emptyBody) {
    const card = document.createElement('div');
    card.className = 'card';
    card.innerHTML = `
      <div class="card-header">
        <span class="card-title">${title}</span>
      </div>
      <div class="empty-state">
        ${iconSvg}
        <span class="empty-state-title">${emptyTitle}</span>
        <span class="empty-state-body">${emptyBody}</span>
      </div>
    `;
    return card;
  }

  /** Render the Vault tab page as a DOM element. */
  function render() {
    const page = document.createElement('div');
    page.className = 'tab-page';

    const heading = document.createElement('h1');
    heading.className = 'section-title';
    heading.textContent = 'Vault';
    page.appendChild(heading);

    // Active sessions card
    page.appendChild(emptyCard(
      'Active Sessions',
      `<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <rect x="3" y="3" width="18" height="18" rx="2"/>
        <path d="M7 10 l2 2 -2 2" stroke-width="1.75"/>
        <line x1="12" y1="14" x2="16" y2="14" stroke-width="1.5"/>
      </svg>`,
      'No active sessions',
      'Connect to a server from the Connections tab'
    ));

    // Tunnels card
    page.appendChild(emptyCard(
      'Tunnels',
      `<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M3 12 Q6 6 12 6 Q18 6 21 12 Q18 18 12 18 Q6 18 3 12"/>
        <circle cx="12" cy="12" r="3"/>
      </svg>`,
      'No active tunnels',
      'Tunnels will appear here when a session is active'
    ));

    // Transfers card
    page.appendChild(emptyCard(
      'Transfers',
      `<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M12 3v12M7 10l5 5 5-5"/>
        <path d="M5 20h14" stroke-width="1.75"/>
      </svg>`,
      'No transfers',
      'File uploads and downloads will appear here'
    ));

    return page;
  }

  exports.vaultTab = { render };
})(window);
