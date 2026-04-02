/**
 * Tmux ID Map — bidirectional mapping between tmux IDs and frontend IDs.
 *
 * In tmux mode, tab and pane creation is driven by tmux notifications.
 * This module tracks which tmux window/pane IDs correspond to which
 * frontend tab/pane IDs.
 */
(function initConchTmuxIdMap(global) {
  'use strict';

  function create() {
    const windowToTab = new Map();
    const tabToWindow = new Map();
    const tmuxToPane = new Map();
    const paneToTmux = new Map();

    function addWindow(tmuxWindowId, frontendTabId) {
      windowToTab.set(tmuxWindowId, frontendTabId);
      tabToWindow.set(frontendTabId, tmuxWindowId);
    }

    function removeWindow(tmuxWindowId) {
      const tabId = windowToTab.get(tmuxWindowId);
      if (tabId !== undefined) {
        tabToWindow.delete(tabId);
      }
      windowToTab.delete(tmuxWindowId);
    }

    function removeWindowByTab(frontendTabId) {
      const windowId = tabToWindow.get(frontendTabId);
      if (windowId !== undefined) {
        windowToTab.delete(windowId);
      }
      tabToWindow.delete(frontendTabId);
    }

    function getTabForWindow(tmuxWindowId) {
      return windowToTab.get(tmuxWindowId);
    }

    function getWindowForTab(frontendTabId) {
      return tabToWindow.get(frontendTabId);
    }

    function addPane(tmuxPaneId, frontendPaneId) {
      tmuxToPane.set(tmuxPaneId, frontendPaneId);
      paneToTmux.set(frontendPaneId, tmuxPaneId);
    }

    function removePane(tmuxPaneId) {
      const paneId = tmuxToPane.get(tmuxPaneId);
      if (paneId !== undefined) {
        paneToTmux.delete(paneId);
      }
      tmuxToPane.delete(tmuxPaneId);
    }

    function removePaneByFrontend(frontendPaneId) {
      const tmuxId = paneToTmux.get(frontendPaneId);
      if (tmuxId !== undefined) {
        tmuxToPane.delete(tmuxId);
      }
      paneToTmux.delete(frontendPaneId);
    }

    function getPaneForTmux(tmuxPaneId) {
      return tmuxToPane.get(tmuxPaneId);
    }

    function getTmuxForPane(frontendPaneId) {
      return paneToTmux.get(frontendPaneId);
    }

    function clear() {
      windowToTab.clear();
      tabToWindow.clear();
      tmuxToPane.clear();
      paneToTmux.clear();
    }

    return {
      addWindow,
      removeWindow,
      removeWindowByTab,
      getTabForWindow,
      getWindowForTab,
      addPane,
      removePane,
      removePaneByFrontend,
      getPaneForTmux,
      getTmuxForPane,
      clear,
    };
  }

  global.conchTmuxIdMap = { create };
})(typeof window !== 'undefined' ? window : globalThis);
