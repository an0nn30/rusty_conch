// SSH Panel — server tree, quick connect, session management.
//
// Depends on: invoke, listen (Tauri APIs) and createSshTab (from main app).

(function (exports) {
  'use strict';

  let invoke = null;
  let listen = null;
  let createSshTabFn = null;
  let panelEl = null;
  let serverListEl = null;
  let quickConnectEl = null;
  let sessionListEl = null;

  // Cached server data
  let serverData = { folders: [], ungrouped: [], ssh_config: [] };

  function init(opts) {
    invoke = opts.invoke;
    listen = opts.listen;
    createSshTabFn = opts.createSshTab;
    panelEl = opts.panelEl;

    panelEl.innerHTML = `
      <div class="ssh-panel-header">
        <span class="ssh-panel-title">Sessions</span>
        <div class="ssh-panel-actions">
          <button class="ssh-icon-btn" id="ssh-add-server" title="Add Server">+</button>
          <button class="ssh-icon-btn" id="ssh-add-folder" title="Add Folder">&#128193;</button>
          <button class="ssh-icon-btn" id="ssh-refresh" title="Refresh">&#8635;</button>
        </div>
      </div>
      <div class="ssh-quick-connect">
        <input type="text" id="ssh-quick-connect-input" placeholder="user@host:port" spellcheck="false" />
      </div>
      <div class="ssh-active-sessions" id="ssh-active-sessions"></div>
      <div class="ssh-server-list" id="ssh-server-list"></div>
    `;

    serverListEl = panelEl.querySelector('#ssh-server-list');
    quickConnectEl = panelEl.querySelector('#ssh-quick-connect-input');
    sessionListEl = panelEl.querySelector('#ssh-active-sessions');

    // Quick connect
    quickConnectEl.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        const spec = quickConnectEl.value.trim();
        if (spec) {
          quickConnectEl.value = '';
          createSshTabFn({ spec });
        }
      }
    });

    // Buttons
    panelEl.querySelector('#ssh-add-server').addEventListener('click', showAddServerDialog);
    panelEl.querySelector('#ssh-add-folder').addEventListener('click', showAddFolderDialog);
    panelEl.querySelector('#ssh-refresh').addEventListener('click', refreshAll);

    // Listen for session changes
    listen('ssh-host-key-prompt', handleHostKeyPrompt);
    listen('ssh-password-prompt', handlePasswordPrompt);

    refreshAll();
  }

  async function refreshAll() {
    try {
      serverData = await invoke('remote_get_servers');
    } catch (e) {
      console.error('Failed to load servers:', e);
      serverData = { folders: [], ungrouped: [], ssh_config: [] };
    }
    renderServerList();
    await refreshSessions();
  }

  async function refreshSessions() {
    try {
      const sessions = await invoke('remote_get_sessions');
      renderSessions(sessions);
    } catch (e) {
      console.error('Failed to load sessions:', e);
    }
  }

  // ---------------------------------------------------------------------------
  // Rendering
  // ---------------------------------------------------------------------------

  function renderServerList() {
    const frag = document.createDocumentFragment();

    // Folders
    for (const folder of serverData.folders) {
      const folderEl = createFolderNode(folder);
      frag.appendChild(folderEl);
    }

    // Ungrouped
    for (const server of serverData.ungrouped) {
      frag.appendChild(createServerNode(server));
    }

    // SSH config imports (dimmed section)
    if (serverData.ssh_config.length > 0) {
      const header = document.createElement('div');
      header.className = 'ssh-section-header';
      header.textContent = '~/.ssh/config';
      frag.appendChild(header);
      for (const server of serverData.ssh_config) {
        frag.appendChild(createServerNode(server, true));
      }
    }

    serverListEl.innerHTML = '';
    serverListEl.appendChild(frag);
  }

  function createFolderNode(folder) {
    const el = document.createElement('div');
    el.className = 'ssh-folder';

    const header = document.createElement('div');
    header.className = 'ssh-folder-header';
    header.innerHTML = `<span class="ssh-folder-arrow">${folder.expanded !== false ? '▼' : '▶'}</span>
      <span class="ssh-folder-name">${esc(folder.name)}</span>
      <span class="ssh-folder-count">${folder.entries.length}</span>`;
    header.addEventListener('click', () => {
      const isExpanded = folder.expanded !== false;
      invoke('remote_set_folder_expanded', { folderId: folder.id, expanded: !isExpanded }).catch(() => {});
      folder.expanded = !isExpanded;
      renderServerList();
    });
    el.appendChild(header);

    if (folder.expanded !== false) {
      const list = document.createElement('div');
      list.className = 'ssh-folder-entries';
      for (const server of folder.entries) {
        list.appendChild(createServerNode(server));
      }
      el.appendChild(list);
    }

    return el;
  }

  function createServerNode(server, dimmed) {
    const el = document.createElement('div');
    el.className = 'ssh-server-node' + (dimmed ? ' dimmed' : '');
    el.title = `${server.user}@${server.host}:${server.port}`;

    const label = server.label || `${server.user}@${server.host}`;
    const detail = server.host + (server.port !== 22 ? ':' + server.port : '');

    el.innerHTML = `<span class="ssh-server-label">${esc(label)}</span>
      <span class="ssh-server-detail">${esc(detail)}</span>`;

    el.addEventListener('dblclick', () => {
      createSshTabFn({ serverId: server.id });
    });

    // Context menu on right click
    el.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      showServerContextMenu(e, server);
    });

    return el;
  }

  function renderSessions(sessions) {
    if (!sessions || sessions.length === 0) {
      sessionListEl.innerHTML = '';
      return;
    }

    const frag = document.createDocumentFragment();
    const header = document.createElement('div');
    header.className = 'ssh-section-header';
    header.textContent = 'Active';
    frag.appendChild(header);

    for (const s of sessions) {
      const el = document.createElement('div');
      el.className = 'ssh-session-node';
      el.innerHTML = `<span class="ssh-session-dot"></span>
        <span class="ssh-session-label">${esc(s.user)}@${esc(s.host)}</span>`;
      frag.appendChild(el);
    }

    sessionListEl.innerHTML = '';
    sessionListEl.appendChild(frag);
  }

  // ---------------------------------------------------------------------------
  // Dialogs (simple prompt-based for now)
  // ---------------------------------------------------------------------------

  function showAddServerDialog() {
    const host = prompt('Host (e.g. example.com):');
    if (!host) return;

    const user = prompt('Username:', getCurrentUser());
    if (!user) return;

    const portStr = prompt('Port:', '22');
    const port = parseInt(portStr, 10) || 22;

    const label = prompt('Label (optional):', `${user}@${host}`);

    const entry = {
      id: crypto.randomUUID(),
      label: label || `${user}@${host}`,
      host,
      port,
      user,
      auth_method: 'key',
      key_path: null,
      proxy_command: null,
      proxy_jump: null,
    };

    invoke('remote_save_server', { entry, folderId: null })
      .then(() => refreshAll())
      .catch((e) => alert('Failed to save server: ' + e));
  }

  function showAddFolderDialog() {
    const name = prompt('Folder name:');
    if (!name) return;

    invoke('remote_add_folder', { name })
      .then(() => refreshAll())
      .catch((e) => alert('Failed to create folder: ' + e));
  }

  function showServerContextMenu(event, server) {
    // Remove existing context menu
    removeContextMenu();

    const menu = document.createElement('div');
    menu.className = 'ssh-context-menu';
    menu.style.left = event.clientX + 'px';
    menu.style.top = event.clientY + 'px';

    const items = [
      { label: 'Connect', action: () => createSshTabFn({ serverId: server.id }) },
      { label: 'Edit', action: () => editServer(server) },
      { label: 'Duplicate', action: () => duplicateServer(server.id) },
      { label: 'Delete', action: () => deleteServer(server) },
    ];

    for (const item of items) {
      const el = document.createElement('div');
      el.className = 'ssh-context-menu-item';
      el.textContent = item.label;
      el.addEventListener('click', () => {
        removeContextMenu();
        item.action();
      });
      menu.appendChild(el);
    }

    document.body.appendChild(menu);

    // Close on click outside
    setTimeout(() => {
      document.addEventListener('click', removeContextMenu, { once: true });
    }, 0);
  }

  function removeContextMenu() {
    const existing = document.querySelector('.ssh-context-menu');
    if (existing) existing.remove();
  }

  function editServer(server) {
    const host = prompt('Host:', server.host);
    if (!host) return;
    const user = prompt('Username:', server.user);
    if (!user) return;
    const portStr = prompt('Port:', String(server.port));
    const port = parseInt(portStr, 10) || 22;
    const label = prompt('Label:', server.label);

    const entry = { ...server, host, user, port, label: label || `${user}@${host}` };
    invoke('remote_save_server', { entry, folderId: null })
      .then(() => refreshAll())
      .catch((e) => alert('Failed to update server: ' + e));
  }

  function duplicateServer(serverId) {
    invoke('remote_duplicate_server', { serverId })
      .then(() => refreshAll())
      .catch((e) => alert('Failed to duplicate: ' + e));
  }

  function deleteServer(server) {
    if (!confirm(`Delete "${server.label}"?`)) return;
    invoke('remote_delete_server', { serverId: server.id })
      .then(() => refreshAll())
      .catch((e) => alert('Failed to delete: ' + e));
  }

  // ---------------------------------------------------------------------------
  // Auth prompts
  // ---------------------------------------------------------------------------

  function handleHostKeyPrompt(event) {
    const { prompt_id, message, detail } = event.payload;
    const accepted = confirm(message + '\n\n' + detail + '\n\nAccept and save?');
    invoke('auth_respond_host_key', { promptId: prompt_id, accepted }).catch(() => {});
  }

  function handlePasswordPrompt(event) {
    const { prompt_id, message } = event.payload;
    const password = prompt(message);
    invoke('auth_respond_password', { promptId: prompt_id, password: password || null }).catch(() => {});
  }

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  function esc(str) {
    const el = document.createElement('span');
    el.textContent = str;
    return el.innerHTML;
  }

  function getCurrentUser() {
    // No way to get $USER from JS, but the backend defaults to it anyway
    return 'root';
  }

  exports.sshPanel = { init, refreshAll, refreshSessions };
})(window);
