// Vault Management — unlock dialog, setup dialog, account CRUD, settings.

(function (exports) {
  'use strict';

  let invoke = null;
  let listen = null;

  // Cached state
  let cachedAccounts = [];
  let lockTimerInterval = null;

  const esc = window.utils.esc;
  const attr = window.utils.attr;

  function init(opts) {
    invoke = opts.invoke;
    listen = opts.listen;

    // Listen for menu-driven vault events.
    listen('vault-locked', () => {
      // Auto-lock fired from backend — dismiss any vault dialogs and clear cache.
      cachedAccounts = [];
      stopLockTimer();
      const overlay = document.getElementById('vault-overlay');
      if (overlay) overlay.remove();
      window.toast.info('Vault Locked', 'The credential vault has been locked.');
    });
  }

  // ---------------------------------------------------------------------------
  // ensureUnlocked — check status, prompt if needed, then call callback
  // ---------------------------------------------------------------------------

  async function ensureUnlocked(callback) {
    try {
      const status = await invoke('vault_status');
      if (!status.exists) {
        showSetupDialog(() => {
          if (callback) callback();
        });
        return;
      }
      if (status.locked) {
        showUnlockDialog(() => {
          if (callback) callback();
        });
        return;
      }
      // Already unlocked.
      if (callback) callback();
    } catch (e) {
      window.toast.error('Vault Error', 'Failed to check vault status: ' + e);
    }
  }

  // ---------------------------------------------------------------------------
  // Setup dialog — first-time vault creation
  // ---------------------------------------------------------------------------

  function showSetupDialog(onSuccess) {
    removeOverlay();

    const overlay = document.createElement('div');
    overlay.className = 'ssh-overlay';
    overlay.id = 'vault-overlay';

    overlay.innerHTML = `
      <div class="ssh-form vault-setup-dialog">
        <div class="ssh-form-title">Create Credential Vault</div>
        <div class="ssh-form-body">
          <p class="vault-description">
            The credential vault securely stores SSH credentials using AES-256-GCM
            encryption with an Argon2id-derived key. Choose a strong master password.
          </p>
          <label class="ssh-form-label">Master Password
            <input type="password" id="vault-setup-pw" placeholder="Enter master password"
                   spellcheck="false" autocomplete="off" />
          </label>
          <label class="ssh-form-label">Confirm Password
            <input type="password" id="vault-setup-pw-confirm" placeholder="Confirm master password"
                   spellcheck="false" autocomplete="off" />
          </label>
          <label class="vault-checkbox-label">
            <input type="checkbox" id="vault-setup-keychain" />
            Store in system keychain for biometric unlock (Touch ID)
          </label>
        </div>
        <div class="ssh-form-buttons">
          <button class="ssh-form-btn" id="vault-setup-cancel">Cancel</button>
          <button class="ssh-form-btn primary" id="vault-setup-create">Create Vault</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);
    setTimeout(() => overlay.querySelector('#vault-setup-pw').focus(), 50);

    overlay.addEventListener('mousedown', (e) => {
      if (e.target === overlay) removeOverlay();
    });

    const onKey = (e) => {
      if (e.key === 'Escape') {
        removeOverlay();
        document.removeEventListener('keydown', onKey, true);
      }
    };
    document.addEventListener('keydown', onKey, true);

    overlay.querySelector('#vault-setup-cancel').addEventListener('click', () => {
      removeOverlay();
      document.removeEventListener('keydown', onKey, true);
    });

    overlay.querySelector('#vault-setup-create').addEventListener('click', async () => {
      const pw = overlay.querySelector('#vault-setup-pw').value;
      const confirm = overlay.querySelector('#vault-setup-pw-confirm').value;
      const enableKeychain = overlay.querySelector('#vault-setup-keychain').checked;

      if (!pw) {
        window.toast.warn('Vault', 'Master password is required.');
        overlay.querySelector('#vault-setup-pw').focus();
        return;
      }
      if (pw.length < 8) {
        window.toast.warn('Vault', 'Password must be at least 8 characters.');
        overlay.querySelector('#vault-setup-pw').focus();
        return;
      }
      if (pw !== confirm) {
        window.toast.warn('Vault', 'Passwords do not match.');
        overlay.querySelector('#vault-setup-pw-confirm').focus();
        return;
      }

      try {
        await invoke('vault_create', { request: { password: pw, enable_keychain: enableKeychain } });
        removeOverlay();
        document.removeEventListener('keydown', onKey, true);
        window.toast.success('Vault Created', 'Your credential vault is ready.');
        if (onSuccess) onSuccess();
      } catch (e) {
        window.toast.error('Vault Error', 'Failed to create vault: ' + e);
      }
    });
  }

  // ---------------------------------------------------------------------------
  // Unlock dialog — master password input
  // ---------------------------------------------------------------------------

  function showUnlockDialog(onSuccess) {
    removeOverlay();

    const overlay = document.createElement('div');
    overlay.className = 'ssh-overlay';
    overlay.id = 'vault-overlay';

    overlay.innerHTML = `
      <div class="ssh-form vault-unlock-dialog">
        <div class="ssh-form-title">Unlock Vault</div>
        <div class="ssh-form-body">
          <label class="ssh-form-label">Master Password
            <input type="password" id="vault-unlock-pw" placeholder="Enter master password"
                   spellcheck="false" autocomplete="off" />
          </label>
        </div>
        <div class="ssh-form-buttons">
          <button class="ssh-form-btn" id="vault-unlock-cancel">Cancel</button>
          <button class="ssh-form-btn primary" id="vault-unlock-submit">Unlock</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);
    setTimeout(() => overlay.querySelector('#vault-unlock-pw').focus(), 50);

    overlay.addEventListener('mousedown', (e) => {
      if (e.target === overlay) removeOverlay();
    });

    const onKey = (e) => {
      if (e.key === 'Escape') {
        removeOverlay();
        document.removeEventListener('keydown', onKey, true);
      }
    };
    document.addEventListener('keydown', onKey, true);

    overlay.querySelector('#vault-unlock-cancel').addEventListener('click', () => {
      removeOverlay();
      document.removeEventListener('keydown', onKey, true);
    });

    const submitUnlock = async () => {
      const pw = overlay.querySelector('#vault-unlock-pw').value;
      if (!pw) {
        window.toast.warn('Vault', 'Password is required.');
        overlay.querySelector('#vault-unlock-pw').focus();
        return;
      }

      try {
        await invoke('vault_unlock', { request: { password: pw } });
        removeOverlay();
        document.removeEventListener('keydown', onKey, true);
        window.toast.success('Vault Unlocked', 'Credential vault is now unlocked.');
        if (onSuccess) onSuccess();
      } catch (e) {
        window.toast.error('Unlock Failed', String(e));
        overlay.querySelector('#vault-unlock-pw').value = '';
        overlay.querySelector('#vault-unlock-pw').focus();
      }
    };

    overlay.querySelector('#vault-unlock-submit').addEventListener('click', submitUnlock);
    overlay.querySelector('#vault-unlock-pw').addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); submitUnlock(); }
    });
  }

  // ---------------------------------------------------------------------------
  // Vault management dialog — sidebar with Accounts / SSH Keys / Settings
  // ---------------------------------------------------------------------------

  const VAULT_SECTIONS = [
    { id: 'accounts', label: 'User Accounts' },
    { id: 'keys', label: 'SSH Keys' },
    { id: 'settings', label: 'Settings' },
  ];

  let currentSection = 'accounts';

  async function showVaultDialog() {
    // Ensure vault is unlocked first.
    const status = await invoke('vault_status').catch(() => null);
    if (!status) return;

    if (!status.exists) {
      showSetupDialog(() => showVaultDialog());
      return;
    }
    if (status.locked) {
      showUnlockDialog(() => showVaultDialog());
      return;
    }

    currentSection = 'accounts';
    await renderVaultDialog();
  }

  async function renderVaultDialog() {
    removeOverlay();

    // Load data for current section.
    let accounts = [];
    let settings = null;
    try {
      accounts = await invoke('vault_list_accounts');
      settings = await invoke('vault_get_settings');
      cachedAccounts = accounts;
    } catch (e) {
      window.toast.error('Vault Error', 'Failed to load vault data: ' + e);
      return;
    }

    const status = await invoke('vault_status').catch(() => ({ exists: true, locked: false, seconds_remaining: 0 }));

    const overlay = document.createElement('div');
    overlay.className = 'ssh-overlay';
    overlay.id = 'vault-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'ssh-form vault-dialog';

    // Title
    const titleEl = document.createElement('div');
    titleEl.className = 'ssh-form-title';
    titleEl.textContent = 'Credential Vault';
    dialog.appendChild(titleEl);

    // Body = sidebar + content
    const body = document.createElement('div');
    body.className = 'vault-body';

    // Sidebar
    const sidebar = document.createElement('div');
    sidebar.className = 'vault-sidebar';

    for (const sec of VAULT_SECTIONS) {
      const item = document.createElement('div');
      item.className = 'vault-sidebar-item' + (sec.id === currentSection ? ' active' : '');
      item.textContent = sec.label;
      item.addEventListener('click', () => {
        currentSection = sec.id;
        renderVaultDialog();
      });
      sidebar.appendChild(item);
    }

    // Sidebar footer — lock status + lock button
    const footer = document.createElement('div');
    footer.className = 'vault-sidebar-footer';
    footer.innerHTML = `
      <div class="vault-lock-status">
        <span class="vault-status-dot unlocked"></span>
        <span id="vault-lock-countdown">${formatCountdown(status.seconds_remaining)}</span>
      </div>
      <button class="vault-lock-btn" id="vault-lock-now">Lock Now</button>
    `;
    sidebar.appendChild(footer);

    body.appendChild(sidebar);

    // Content area
    const content = document.createElement('div');
    content.className = 'vault-content';
    content.id = 'vault-content';

    if (currentSection === 'accounts') {
      renderAccountsSection(content, accounts);
    } else if (currentSection === 'keys') {
      renderKeysSection(content);
    } else if (currentSection === 'settings') {
      renderSettingsSection(content, settings);
    }

    body.appendChild(content);
    dialog.appendChild(body);

    // Footer buttons
    const buttons = document.createElement('div');
    buttons.className = 'ssh-form-buttons';
    buttons.innerHTML = '<button class="ssh-form-btn" id="vault-close">Close</button>';
    dialog.appendChild(buttons);

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    // Start countdown timer.
    startLockTimer(overlay);

    // Events
    overlay.addEventListener('mousedown', (e) => {
      if (e.target === overlay) { stopLockTimer(); removeOverlay(); }
    });

    const onKey = (e) => {
      if (e.key === 'Escape') {
        stopLockTimer();
        removeOverlay();
        document.removeEventListener('keydown', onKey, true);
      }
    };
    document.addEventListener('keydown', onKey, true);

    overlay.querySelector('#vault-close').addEventListener('click', () => {
      stopLockTimer();
      removeOverlay();
      document.removeEventListener('keydown', onKey, true);
    });

    overlay.querySelector('#vault-lock-now').addEventListener('click', async () => {
      try {
        await invoke('vault_lock');
        stopLockTimer();
        removeOverlay();
        document.removeEventListener('keydown', onKey, true);
        cachedAccounts = [];
        window.toast.info('Vault Locked', 'Credential vault has been locked.');
      } catch (e) {
        window.toast.error('Vault Error', 'Failed to lock vault: ' + e);
      }
    });
  }

  // ---------------------------------------------------------------------------
  // Accounts section
  // ---------------------------------------------------------------------------

  function renderAccountsSection(container, accounts) {
    let html = '<div class="vault-section-header">';
    html += '<h3>User Accounts</h3>';
    html += '<button class="ssh-form-btn primary vault-add-btn" id="vault-add-account">New Account</button>';
    html += '</div>';

    if (accounts.length === 0) {
      html += '<div class="vault-empty">No accounts yet. Create one to store SSH credentials.</div>';
    } else {
      html += '<div class="vault-account-list">';
      for (const acct of accounts) {
        const initials = getInitials(acct.display_name);
        const authLabel = formatAuthType(acct.auth_type);
        html += `
          <div class="vault-account-row" data-id="${attr(acct.id)}">
            <div class="vault-account-avatar">${esc(initials)}</div>
            <div class="vault-account-info">
              <div class="vault-account-name">${esc(acct.display_name)}</div>
              <div class="vault-account-detail">${esc(acct.username)} &middot; ${esc(authLabel)}</div>
            </div>
            <div class="vault-account-actions">
              <button class="vault-row-btn vault-edit-btn" data-id="${attr(acct.id)}" title="Edit">Edit</button>
              <button class="vault-row-btn vault-delete-btn danger" data-id="${attr(acct.id)}" title="Delete">Delete</button>
            </div>
          </div>
        `;
      }
      html += '</div>';
    }

    container.innerHTML = html;

    // Wire add button.
    const addBtn = container.querySelector('#vault-add-account');
    if (addBtn) {
      addBtn.addEventListener('click', () => showAccountForm(null));
    }

    // Wire edit/delete buttons.
    container.querySelectorAll('.vault-edit-btn').forEach((btn) => {
      btn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const id = btn.dataset.id;
        try {
          const account = await invoke('vault_get_account', { id });
          showAccountForm(account);
        } catch (err) {
          window.toast.error('Vault Error', 'Failed to load account: ' + err);
        }
      });
    });

    container.querySelectorAll('.vault-delete-btn').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (btn.dataset.confirm !== 'yes') {
          btn.dataset.confirm = 'yes';
          btn.textContent = 'Confirm?';
          btn.classList.add('confirm');
          setTimeout(() => {
            if (btn.isConnected) {
              btn.dataset.confirm = '';
              btn.textContent = 'Delete';
              btn.classList.remove('confirm');
            }
          }, 3000);
          return;
        }
        const id = btn.dataset.id;
        invoke('vault_delete_account', { id })
          .then(() => {
            window.toast.success('Deleted', 'Account removed from vault.');
            renderVaultDialog();
          })
          .catch((err) => window.toast.error('Delete Failed', String(err)));
      });
    });
  }

  // ---------------------------------------------------------------------------
  // Keys section (placeholder)
  // ---------------------------------------------------------------------------

  function renderKeysSection(container) {
    container.innerHTML = `
      <div class="vault-section-header">
        <h3>SSH Keys</h3>
        <button class="ssh-form-btn primary vault-add-btn" id="vault-gen-key">Generate Key</button>
      </div>
      <div class="vault-empty">
        SSH key management will list generated keys stored in the vault.
        Use the Generate Key button or Tools &gt; Generate SSH Key to create a new key pair.
      </div>
    `;

    container.querySelector('#vault-gen-key').addEventListener('click', () => {
      if (window.keygen) {
        window.keygen.showKeygenDialog({ linkToVault: true });
      } else {
        window.toast.info('Coming Soon', 'Key generation dialog is not yet available.');
      }
    });
  }

  // ---------------------------------------------------------------------------
  // Settings section
  // ---------------------------------------------------------------------------

  function renderSettingsSection(container, settings) {
    if (!settings) {
      container.innerHTML = '<div class="vault-empty">Failed to load vault settings.</div>';
      return;
    }

    const autoSaveOptions = ['Always', 'Ask', 'Never'];

    container.innerHTML = `
      <div class="vault-section-header"><h3>Vault Settings</h3></div>
      <div class="vault-settings-form">
        <label class="ssh-form-label">Auto-Lock Timeout (minutes)
          <input type="number" id="vault-setting-timeout" value="${settings.auto_lock_minutes}"
                 min="1" max="1440" />
        </label>
        <label class="vault-checkbox-label">
          <input type="checkbox" id="vault-setting-agent" ${settings.push_to_system_agent ? 'checked' : ''} />
          Push keys to system SSH agent on unlock
        </label>
        <label class="vault-checkbox-label">
          <input type="checkbox" id="vault-setting-keychain" ${settings.os_keychain_enabled ? 'checked' : ''} />
          Enable OS keychain for biometric unlock
        </label>
        <label class="ssh-form-label">Auto-Save Passwords
          <select id="vault-setting-autosave">
            ${autoSaveOptions.map((opt) =>
              '<option value="' + attr(opt) + '"' +
              (settings.auto_save_passwords === opt ? ' selected' : '') +
              '>' + esc(opt) + '</option>'
            ).join('')}
          </select>
        </label>
        <div class="vault-settings-actions">
          <button class="ssh-form-btn primary" id="vault-save-settings">Save Settings</button>
        </div>
      </div>
    `;

    container.querySelector('#vault-save-settings').addEventListener('click', async () => {
      const timeout = parseInt(container.querySelector('#vault-setting-timeout').value, 10);
      const agent = container.querySelector('#vault-setting-agent').checked;
      const keychain = container.querySelector('#vault-setting-keychain').checked;
      const autoSave = container.querySelector('#vault-setting-autosave').value;

      if (!timeout || timeout < 1 || timeout > 1440) {
        window.toast.warn('Invalid', 'Timeout must be between 1 and 1440 minutes.');
        return;
      }

      const updated = {
        auto_lock_minutes: timeout,
        push_to_system_agent: agent,
        os_keychain_enabled: keychain,
        auto_save_passwords: autoSave,
      };

      try {
        await invoke('vault_update_settings', { settings: updated });
        window.toast.success('Settings Saved', 'Vault settings updated.');
      } catch (e) {
        window.toast.error('Settings Error', 'Failed to save settings: ' + e);
      }
    });
  }

  // ---------------------------------------------------------------------------
  // Account form — create / edit
  // ---------------------------------------------------------------------------

  function showAccountForm(existing) {
    removeOverlay();

    const isEdit = existing != null;
    const title = isEdit ? 'Edit Account' : 'New Account';

    const displayName = existing ? existing.display_name : '';
    const username = existing ? existing.username : '';
    const authType = existing ? existing.auth_type : 'password';
    const keyPath = existing ? (existing.key_path || '') : '';

    const overlay = document.createElement('div');
    overlay.className = 'ssh-overlay';
    overlay.id = 'vault-overlay';
    overlay.style.zIndex = '3100';

    overlay.innerHTML = `
      <div class="ssh-form vault-account-form">
        <div class="ssh-form-title">${esc(title)}</div>
        <div class="ssh-form-body">
          <label class="ssh-form-label">Display Name
            <input type="text" id="vault-acct-name" value="${attr(displayName)}"
                   placeholder="e.g. Production Deploy Key" spellcheck="false" />
          </label>
          <label class="ssh-form-label">Username
            <input type="text" id="vault-acct-user" value="${attr(username)}"
                   placeholder="e.g. root, deploy, ubuntu" spellcheck="false" autocomplete="off" />
          </label>
          <label class="ssh-form-label">Authentication Method
            <select id="vault-acct-auth">
              <option value="password" ${authType === 'password' ? 'selected' : ''}>Password</option>
              <option value="key" ${authType === 'key' ? 'selected' : ''}>SSH Key</option>
              <option value="key_and_password" ${authType === 'key_and_password' ? 'selected' : ''}>SSH Key + Password</option>
            </select>
          </label>
          <div id="vault-acct-pw-fields" style="${authType === 'key' ? 'display:none' : ''}">
            <label class="ssh-form-label">Password
              <input type="password" id="vault-acct-pw" placeholder="${isEdit ? '(unchanged if empty)' : 'Enter password'}"
                     spellcheck="false" autocomplete="off" />
            </label>
          </div>
          <div id="vault-acct-key-fields" style="${authType === 'password' ? 'display:none' : ''}">
            <label class="ssh-form-label">Key File Path
              <input type="text" id="vault-acct-keypath" value="${attr(keyPath)}"
                     placeholder="~/.ssh/id_ed25519" spellcheck="false" />
            </label>
            <label class="ssh-form-label">Key Passphrase (optional)
              <input type="password" id="vault-acct-passphrase"
                     placeholder="${isEdit ? '(unchanged if empty)' : 'Enter passphrase'}"
                     spellcheck="false" autocomplete="off" />
            </label>
          </div>
        </div>
        <div class="ssh-form-buttons">
          <button class="ssh-form-btn" id="vault-acct-cancel">Cancel</button>
          <button class="ssh-form-btn primary" id="vault-acct-save">${isEdit ? 'Save Changes' : 'Create Account'}</button>
        </div>
      </div>
    `;

    document.body.appendChild(overlay);
    setTimeout(() => overlay.querySelector('#vault-acct-name').focus(), 50);

    // Toggle field visibility based on auth type.
    const authSelect = overlay.querySelector('#vault-acct-auth');
    authSelect.addEventListener('change', () => {
      const val = authSelect.value;
      overlay.querySelector('#vault-acct-pw-fields').style.display =
        val === 'key' ? 'none' : '';
      overlay.querySelector('#vault-acct-key-fields').style.display =
        val === 'password' ? 'none' : '';
    });

    overlay.addEventListener('mousedown', (e) => {
      if (e.target === overlay) closeAccountForm(overlay);
    });

    const onKey = (e) => {
      if (e.key === 'Escape') {
        closeAccountForm(overlay);
        document.removeEventListener('keydown', onKey, true);
      }
    };
    document.addEventListener('keydown', onKey, true);

    overlay.querySelector('#vault-acct-cancel').addEventListener('click', () => {
      closeAccountForm(overlay);
      document.removeEventListener('keydown', onKey, true);
    });

    overlay.querySelector('#vault-acct-save').addEventListener('click', async () => {
      const name = overlay.querySelector('#vault-acct-name').value.trim();
      const user = overlay.querySelector('#vault-acct-user').value.trim();
      const auth = overlay.querySelector('#vault-acct-auth').value;
      const pw = overlay.querySelector('#vault-acct-pw').value;
      const kp = overlay.querySelector('#vault-acct-keypath').value.trim();
      const passphrase = overlay.querySelector('#vault-acct-passphrase').value;

      if (!name) {
        window.toast.warn('Vault', 'Display name is required.');
        overlay.querySelector('#vault-acct-name').focus();
        return;
      }
      if (!user) {
        window.toast.warn('Vault', 'Username is required.');
        overlay.querySelector('#vault-acct-user').focus();
        return;
      }

      if (auth === 'password' && !isEdit && !pw) {
        window.toast.warn('Vault', 'Password is required for password auth.');
        overlay.querySelector('#vault-acct-pw').focus();
        return;
      }
      if ((auth === 'key' || auth === 'key_and_password') && !kp) {
        window.toast.warn('Vault', 'Key file path is required.');
        overlay.querySelector('#vault-acct-keypath').focus();
        return;
      }
      if (auth === 'key_and_password' && !isEdit && !pw) {
        window.toast.warn('Vault', 'Password is required for key+password auth.');
        overlay.querySelector('#vault-acct-pw').focus();
        return;
      }

      try {
        if (isEdit) {
          const req = {
            id: existing.id,
            display_name: name,
            username: user,
            auth_type: auth,
            password: pw || null,
            key_path: (auth === 'key' || auth === 'key_and_password') ? kp : null,
            passphrase: passphrase || null,
          };
          await invoke('vault_update_account', { request: req });
          window.toast.success('Updated', 'Account updated successfully.');
        } else {
          const req = {
            display_name: name,
            username: user,
            auth_type: auth,
            password: (auth === 'password' || auth === 'key_and_password') ? pw : null,
            key_path: (auth === 'key' || auth === 'key_and_password') ? kp : null,
            passphrase: passphrase || null,
          };
          await invoke('vault_add_account', { request: req });
          window.toast.success('Created', 'Account added to vault.');
        }

        closeAccountForm(overlay);
        document.removeEventListener('keydown', onKey, true);
        renderVaultDialog();
      } catch (e) {
        window.toast.error('Save Failed', String(e));
      }
    });
  }

  function closeAccountForm(overlay) {
    if (overlay) overlay.remove();
    // Re-show the vault dialog behind it.
    renderVaultDialog();
  }

  // ---------------------------------------------------------------------------
  // getAccounts — return cached account list for external consumers
  // ---------------------------------------------------------------------------

  async function getAccounts() {
    try {
      const status = await invoke('vault_status');
      if (!status.exists || status.locked) return [];
      cachedAccounts = await invoke('vault_list_accounts');
      return cachedAccounts;
    } catch (e) {
      return cachedAccounts;
    }
  }

  // ---------------------------------------------------------------------------
  // Lock timer
  // ---------------------------------------------------------------------------

  function startLockTimer(overlay) {
    stopLockTimer();
    lockTimerInterval = setInterval(async () => {
      try {
        const status = await invoke('vault_status');
        const el = overlay.querySelector('#vault-lock-countdown');
        if (el) el.textContent = formatCountdown(status.seconds_remaining);

        const dot = overlay.querySelector('.vault-status-dot');
        if (dot) {
          dot.className = 'vault-status-dot ' + (status.locked ? 'locked' : 'unlocked');
        }

        if (status.locked) {
          stopLockTimer();
          removeOverlay();
          cachedAccounts = [];
          window.toast.info('Vault Locked', 'The vault was auto-locked due to inactivity.');
        }
      } catch (_) {
        // Ignore polling errors.
      }
    }, 5000);
  }

  function stopLockTimer() {
    if (lockTimerInterval) {
      clearInterval(lockTimerInterval);
      lockTimerInterval = null;
    }
  }

  function formatCountdown(seconds) {
    if (seconds <= 0) return 'Locked';
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return m + ':' + String(s).padStart(2, '0') + ' remaining';
  }

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  function removeOverlay() {
    const el = document.getElementById('vault-overlay');
    if (el) el.remove();
  }

  function getInitials(name) {
    if (!name) return '?';
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[1][0]).toUpperCase();
    }
    return parts[0].substring(0, 2).toUpperCase();
  }

  function formatAuthType(type) {
    switch (type) {
      case 'password': return 'Password';
      case 'key': return 'SSH Key';
      case 'key_and_password': return 'Key + Password';
      default: return type;
    }
  }

  exports.vault = {
    init,
    ensureUnlocked,
    showSetupDialog,
    showUnlockDialog,
    showVaultDialog,
    showAccountForm,
    getAccounts,
  };
})(window);
