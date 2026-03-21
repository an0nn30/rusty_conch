// Profile tab — user avatar, settings groups.

(function (exports) {
  'use strict';

  const CHEVRON = `<svg width="8" height="14" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M1 1l6 6-6 6"/></svg>`;

  function settingsItem(label, right, onTap) {
    const el = document.createElement('div');
    el.className = 'settings-item';
    el.innerHTML = `
      <span class="settings-item-label">${label}</span>
      <span class="settings-item-right">${right}</span>
    `;
    if (onTap) el.addEventListener('click', onTap);
    return el;
  }

  function settingsGroup(title, items) {
    const group = document.createElement('div');
    group.className = 'settings-group';
    if (title) {
      const t = document.createElement('div');
      t.className = 'settings-group-title';
      t.textContent = title;
      group.appendChild(t);
    }
    items.forEach(el => group.appendChild(el));
    return group;
  }

  /** Render the Profile tab page as a DOM element. */
  function render() {
    const page = document.createElement('div');
    page.className = 'tab-page';

    // Avatar + name header
    const header = document.createElement('div');
    header.className = 'profile-header';
    header.innerHTML = `
      <div class="avatar">C</div>
      <div class="profile-name">Conch</div>
      <div class="profile-sub">iOS SSH Client</div>
    `;
    page.appendChild(header);

    // Appearance group
    page.appendChild(settingsGroup('Appearance', [
      settingsItem(
        'Theme',
        `<span class="settings-item-value">Dracula</span>${CHEVRON}`,
        () => window.toast.info('Theme', 'Theme selection coming soon.')
      ),
    ]));

    // Security group
    page.appendChild(settingsGroup('Security', [
      settingsItem(
        'SSH Keys',
        `<span class="settings-item-value">No keys</span>${CHEVRON}`,
        () => window.toast.info('SSH Keys', 'Key management coming soon.')
      ),
    ]));

    // Sync group — iCloud toggle
    const syncToggle = (() => {
      const label = document.createElement('div');
      label.className = 'settings-item';

      const span = document.createElement('span');
      span.className = 'settings-item-label';
      span.textContent = 'iCloud Sync';

      const toggleWrap = document.createElement('label');
      toggleWrap.className = 'toggle';
      toggleWrap.innerHTML = `
        <input type="checkbox" id="icloud-sync-toggle">
        <div class="toggle-track"></div>
        <div class="toggle-thumb"></div>
      `;
      toggleWrap.querySelector('input').addEventListener('change', e => {
        const enabled = e.target.checked;
        window.toast.info('iCloud Sync', enabled ? 'Sync enabled.' : 'Sync disabled.');
      });

      label.appendChild(span);
      label.appendChild(toggleWrap);
      return label;
    })();

    page.appendChild(settingsGroup('Sync', [syncToggle]));

    // Keyboard group
    page.appendChild(settingsGroup('Input', [
      settingsItem(
        'Keyboard',
        `<span class="settings-item-value">Default</span>${CHEVRON}`,
        () => window.toast.info('Keyboard', 'Keyboard settings coming soon.')
      ),
    ]));

    // About group
    page.appendChild(settingsGroup('About', [
      settingsItem(
        'Version',
        `<span class="settings-item-value">0.1.0</span>`,
        null
      ),
      settingsItem(
        'About Conch',
        CHEVRON,
        () => window.toast.info('Conch Mobile', 'An open-source iOS SSH client.')
      ),
    ]));

    return page;
  }

  exports.profileTab = { render };
})(window);
