<script lang="ts">
  import PathSelector from "../PathSelector.svelte";
  import {
    Settings2,
    RefreshCw,
    Folder,
    Save,
    Info,
    Copy,
  } from "lucide-svelte";
  import { addMessage } from "$lib/stores";
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { Window } from "@tauri-apps/api/window";
  import {
    backgroundEnabled,
    cachedVersions,
    catalogLastRefreshed,
    catalogResetNonce,
    currentJokerView,
    currentModView,
    modsStore,
    searchResults,
  } from "../../stores/modStore";
  import { descriptionsStore } from "../../stores/descriptions";
  import { cardScale, darkMode } from "../../stores/ui";
  import { browser } from "$app/environment";

  let isReindexing = $state(false);
  let isClearingCache = $state(false);
  let isConsoleEnabled = $state(false);
  let isBackgroundAnimationEnabled = $state(false);
  let isDarkMode = $state(false);
  let isFullscreen = $state(false);
  let isFullscreenChanging = $state(false);
  let lastReindexStats = $state({
    removedFiles: 0,
    cleanedEntries: 0,
  });
  let isDiscordRpcEnabled = $state(false);
  let isAnalyticsEnabled = $state(true);
  let isLinux = $state(false);
  let linuxPrefix = $state("");
  let isCompatHelperEnabled = $state(false);
  let showLinuxHelp = $state(false);
  let activeHelpImage = $state<{ src: string; alt: string } | null>(null);

  export async function performReindexMods() {
    isReindexing = true;
    try {
      const result = await invoke<[number, number]>("reindex_mods");
      lastReindexStats = {
        removedFiles: result[0], // Will always be 0
        cleanedEntries: result[1],
      };
      addMessage(
        `Reindex complete: Cleaned ${result[1]} database entries`,
        "success",
      );
    } catch (error) {
      addMessage("Reindex failed: " + error, "error");
    } finally {
      isReindexing = false;
    }
  }

  async function clearCache() {
    isClearingCache = true;
    try {
      await invoke("clear_cache");
      // Also wipe on-disk catalog/thumbnail/details directories so a stale
      // entry can't keep crashing the next load.
      try {
        await invoke("clear_app_state");
      } catch (e) {
        console.warn("clear_app_state failed:", e);
      }
      // Also clear small UI caches persisted in localStorage
      try {
        localStorage.removeItem("version-cache-steamodded");
        localStorage.removeItem("version-cache-talisman");
        localStorage.removeItem("mods-cache");
        localStorage.removeItem("mods-cache-ts");
        localStorage.removeItem("mods-descriptions-cache");
      } catch (e) {
        // ignore storage errors
      }
      // Clear in-memory stores so the Mods view refetches fresh data.
      modsStore.set([]);
      searchResults.set([]);
      currentModView.set(null);
      currentJokerView.set(null);
      catalogLastRefreshed.set(null);
      catalogResetNonce.update((n) => n + 1);
      cachedVersions.set({ steamodded: [], talisman: [] });
      descriptionsStore.set({});
      addMessage("Successfully cleared all caches!", "success");
    } catch (error) {
      addMessage("Failed to clear cache: " + error, "error");
    } finally {
      isClearingCache = false;
    }
  }

  async function handleDiscordRpcChange() {
    const newValue = !isDiscordRpcEnabled;
    try {
      await invoke("set_discord_rpc_status", { enabled: newValue });
      isDiscordRpcEnabled = newValue;
      addMessage(
        `Discord Rich Presence ${newValue ? "enabled" : "disabled"}`,
        "success",
      );
    } catch (error) {
      console.error("Failed to set Discord RPC status:", error);
      addMessage("Failed to update Discord Rich Presence status", "error");
    }
  }

  async function handleAnalyticsChange() {
    const newValue = !isAnalyticsEnabled;
    try {
      await invoke("set_analytics_status", { enabled: newValue });
      isAnalyticsEnabled = newValue;
      addMessage(
        `Anonymous usage data ${newValue ? "enabled" : "disabled"}`,
        "success",
      );
    } catch (error) {
      console.error("Failed to set analytics status:", error);
      addMessage("Failed to update analytics setting", "error");
    }
  }

  async function openModsFolder() {
    try {
      const modsPath: string = await invoke("get_mods_folder");
      await invoke("open_directory", { path: modsPath });
    } catch (error) {
      addMessage(`Failed to open mods directory: ${error}`, "error");
    }
  }

  async function handleConsoleChange() {
    const newValue = !isConsoleEnabled;
    try {
      await invoke("set_lovely_console_status", { enabled: newValue });
      isConsoleEnabled = newValue;
      addMessage(
        `Lovely Console ${newValue ? "enabled" : "disabled"}`,
        "success",
      );
    } catch (error) {
      console.error("Failed to set console status:", error);
      addMessage("Failed to update Lovely Console status", "error");
    }
  }

  async function handleBackgroundAnimationChange() {
    const newValue = !isBackgroundAnimationEnabled;

    // Optimistic UI update
    backgroundEnabled.set(newValue);

    try {
      await invoke("set_background_state", { enabled: newValue });
      isBackgroundAnimationEnabled = newValue;
    } catch (error) {
      // Rollback on failure
      backgroundEnabled.set(!newValue);
      isBackgroundAnimationEnabled = !newValue;
    }
  }

  async function handleCompatHelperChange() {
    const newValue = !isCompatHelperEnabled;
    try {
      await invoke("set_compat_helper_status", { enabled: newValue });
      isCompatHelperEnabled = newValue;
      addMessage(
        `Compatibility helper ${newValue ? "enabled" : "disabled"}`,
        "success",
      );
    } catch (error) {
      console.error("Failed to set compatibility helper:", error);
      addMessage("Failed to update compatibility helper", "error");
    }
  }

  async function handleDarkModeChange() {
    const newValue = !isDarkMode;
    darkMode.set(newValue);
  }

  async function handleFullscreenChange() {
    const newValue = !isFullscreen;
    isFullscreen = newValue;
    isFullscreenChanging = true;
    try {
      await Window.getCurrent().setFullscreen(newValue);
    } catch (error) {
      isFullscreen = !newValue;
      console.error("Failed to set fullscreen:", error);
      addMessage("Failed to update fullscreen setting", "error");
    } finally {
      isFullscreenChanging = false;
    }
  }

  async function handleLinuxPrefixChange() {
    const newValue = linuxPrefix.replace(/\s+/g, " ").trim();
    if (newValue && newValue !== linuxPrefix) {
      linuxPrefix = newValue;
      addMessage("Linux prefix had extra spaces and was normalized", "warning");
    }
    try {
      await invoke("set_linux_prefix", { value: newValue });
      if (newValue) {
        addMessage(`Linux prefix set to ${newValue}`, "success");
      } else {
        addMessage("Linux prefix cleared (will use native LOVE)", "success");
      }
    } catch (error) {
      console.error("Failed to set prefix:", error);
      addMessage("Failed to update Linux prefix", "error");
    }
  }

  async function copyLinuxLaunchOptions() {
    const text = 'WINEDLLOVERRIDES="version=n,b" %command%';
    try {
      await navigator.clipboard.writeText(text);
      addMessage("Copied Steam launch options to clipboard", "success");
    } catch (error) {
      console.error("Failed to copy launch options:", error);
      addMessage("Failed to copy Steam launch options", "error");
    }
  }

  function toggleLinuxHelp() {
    showLinuxHelp = !showLinuxHelp;
  }

  function openHelpImage(src: string, alt: string) {
    activeHelpImage = { src, alt };
  }

  function closeHelpImage() {
    activeHelpImage = null;
  }

  function handleModalKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      closeHelpImage();
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      closeHelpImage();
    }
  }

  interface AllSettings {
    discord_rpc: boolean;
    lovely_console: boolean;
    background_enabled: boolean;
    compat_helper: boolean;
    linux_prefix: string;
    launch_mode: string;
    analytics_enabled: boolean;
  }

  onMount(() => {
    const unsubscribeDarkMode = darkMode.subscribe((value) => {
      isDarkMode = value;
    });

    (async () => {
      // Detect platform for Linux-specific UI gating
      try {
        const { platform } = await import("@tauri-apps/plugin-os");
        isLinux = (await platform()) === "linux";
      } catch (_) {
        if (browser) {
          isLinux =
            navigator.userAgent.toLowerCase().includes("linux") &&
            !navigator.userAgent.toLowerCase().includes("android");
        }
      }

      try {
        isFullscreen = await Window.getCurrent().isFullscreen();
      } catch (error) {
        console.warn("Failed to read fullscreen state:", error);
      }

      // Fetch all settings in a single batched IPC call
      try {
        const settings = await invoke<AllSettings>("get_all_settings");
        isDiscordRpcEnabled = settings.discord_rpc;
        isConsoleEnabled = settings.lovely_console;
        isBackgroundAnimationEnabled = settings.background_enabled;
        backgroundEnabled.set(isBackgroundAnimationEnabled);
        isCompatHelperEnabled = settings.compat_helper;
        isAnalyticsEnabled = settings.analytics_enabled;
        if (isLinux) {
          linuxPrefix = settings.linux_prefix;
          if (!linuxPrefix) {
            linuxPrefix = "steam -applaunch 2379780";
            await invoke("set_linux_prefix", { value: linuxPrefix });
            addMessage(
              "Linux prefix defaulted to steam -applaunch 2379780",
              "info",
            );
          }
        }
      } catch (error) {
        console.error("Failed to load settings:", error);
        addMessage("Error loading settings", "error");
      }
    })();

    return () => {
      unsubscribeDarkMode();
    };
  });
</script>

<div class="container default-scrollbar">
  <div class="settings-container">
    <h2>Settings</h2>
    <div class="content">
      <h3>Game Path</h3>
      <PathSelector />
      <h3>Cache</h3>
      <button
        class="clear-cache-button"
        onclick={clearCache}
        disabled={isClearingCache}
      >
        {#if isClearingCache}
          <div class="throbber"></div>
        {:else}
          <RefreshCw size={20} />
          Clear Cache
        {/if}
      </button>

      <p class="description warning">
        <span class="warning-icon">⚠️</span>
        Frequent cache clearing may trigger API rate limits
      </p>

      <h3>Mods</h3>

      <div class="mods-settings">
        <button
          class="open-folder-button"
          onclick={openModsFolder}
          title="Open mods folder"
        >
          <Folder size={20} />
          Open Mods Folder
        </button>

        <p class="description">
          Open the folder where mods are stored on your system.
        </p>
        <div class="console-settings">
          <span class="label-text">Compatibility Helper (Experimental)</span>
          <div class="switch-container">
            <label class="switch">
              <input
                type="checkbox"
                checked={isCompatHelperEnabled}
                onchange={handleCompatHelperChange}
              /> <span class="slider"></span>
            </label>
          </div>
        </div>
        <p class="description-small">
          Enables a hidden Lovely helper that runs compatibility checks before
          mods load.
        </p>

        <button
          class="reindex-button"
          onclick={performReindexMods}
          disabled={isReindexing}
          title="Synchronize database with filesystem state"
        >
          {#if isReindexing}
            <div class="throbber"></div>
            Scanning...
          {:else}
            <Settings2 size={20} />
            Validate Mod Database
          {/if}
        </button>

        {#if lastReindexStats.removedFiles + lastReindexStats.cleanedEntries > 0}
          <div class="reindex-stats">
            <strong>Last cleanup:</strong>
            <span>Files removed: {lastReindexStats.removedFiles}</span>
            <span
              >Database entries cleaned: {lastReindexStats.cleanedEntries}</span
            >
          </div>
        {/if}
        <p class="description-small">
          Performs consistency check on the mod database. Will only remove:
          <br />• Database entries for missing mod installations
        </p>
        {#if isLinux}
          <div class="linux-note">
            <span class="linux-note-icon">
              <Info size={18} />
            </span>
            <div class="linux-note-content">
              <strong>Linux Steam launch:</strong>
              Set Steam launch options for Balatro to
              <code>WINEDLLOVERRIDES="version=n,b" %command%</code> so Lovely
              and mods load when using <code>steam -applaunch 2379780</code>.
              <div class="linux-note-actions">
                <button
                  class="linux-copy-button"
                  onclick={copyLinuxLaunchOptions}
                >
                  <Copy size={16} />
                  Copy launch options
                </button>
                <button class="linux-what-button" onclick={toggleLinuxHelp}>
                  {showLinuxHelp ? "Hide" : "What?"}
                </button>
              </div>
              {#if showLinuxHelp}
                <div class="linux-help">
                  <figure>
                    <button
                      type="button"
                      class="linux-help-button"
                      onclick={() =>
                        openHelpImage(
                          "/images/steam-help-first.png",
                          "Open Balatro properties from Steam library",
                        )}
                      aria-label="Open step 1 help image"
                    >
                      <img
                        src="/images/steam-help-first.png"
                        alt="Open Balatro properties from Steam library"
                        draggable="false"
                      />
                    </button>
                    <figcaption>
                      1. Right-click Balatro → Properties.
                    </figcaption>
                  </figure>
                  <figure>
                    <button
                      type="button"
                      class="linux-help-button"
                      onclick={() =>
                        openHelpImage(
                          "/images/steam-help-1.png",
                          "Steam launch options menu",
                        )}
                      aria-label="Open step 2 help image"
                    >
                      <img
                        src="/images/steam-help-1.png"
                        alt="Steam launch options menu"
                        draggable="false"
                      />
                    </button>
                    <figcaption>2. Find the Launch Options field.</figcaption>
                  </figure>
                  <figure>
                    <button
                      type="button"
                      class="linux-help-button"
                      onclick={() =>
                        openHelpImage(
                          "/images/steam-help-2.png",
                          "Set WINEDLLOVERRIDES launch options",
                        )}
                      aria-label="Open step 3 help image"
                    >
                      <img
                        src="/images/steam-help-2.png"
                        alt="Set WINEDLLOVERRIDES launch options"
                        draggable="false"
                      />
                    </button>
                    <figcaption>3. Paste the WINEDLLOVERRIDES line.</figcaption>
                  </figure>
                </div>
              {/if}
            </div>
          </div>
          {#if activeHelpImage}
            <div
              class="image-modal"
              role="button"
              aria-label="Close image preview"
              tabindex="0"
              onclick={closeHelpImage}
              onkeydown={handleModalKeydown}
              transition:fade={{ duration: 120 }}
            >
              <div
                class="image-modal-content"
                role="presentation"
                onclick={(event) => event.stopPropagation()}
                transition:fly={{ y: 12, duration: 180 }}
              >
                <button class="image-modal-close" onclick={closeHelpImage}>
                  ×
                </button>
                <img
                  src={activeHelpImage.src}
                  alt={activeHelpImage.alt}
                  draggable="false"
                />
                <p>{activeHelpImage.alt}</p>
              </div>
            </div>
          {/if}
          <input
            type="text"
            bind:value={linuxPrefix}
            placeholder="Linux prefix (e.g. proton, protontricks-launch)"
            class="prefix-input"
          />
          <button
            class="prefix-button"
            onclick={handleLinuxPrefixChange}
            title="Update Linux prefix"
          >
            <Save size={20} />
            Save Prefix
          </button>
          <p class="description-small">
            Launch command for Linux (Proton/Wine/Steam). Leave blank to use
            native LOVE. Use `{"{exe}"}` to place the Balatro.exe path, or
            `steam -applaunch 2379780` to launch via Steam. Lovely requires
            `WINEDLLOVERRIDES=version=n,b` and uses the Proton Mods folder
            (linked from your host Mods dir).
          </p>
        {/if}
      </div>
      <h3>Appearance</h3>
      <div class="console-settings">
        <span class="label-text">Enable Dark Mode</span>
        <div class="switch-container">
          <label class="switch">
            <input
              type="checkbox"
              checked={isDarkMode}
              onchange={handleDarkModeChange}
            /> <span class="slider"></span>
          </label>
        </div>
      </div>
      <p class="description-small">
        Use a darker palette across the UI for low-light environments.
      </p>
      <div class="console-settings">
        <span class="label-text">Launch Fullscreen</span>
        <div class="switch-container">
          <label class="switch">
            <input
              type="checkbox"
              aria-label="Fullscreen"
              checked={isFullscreen}
              onchange={handleFullscreenChange}
              disabled={isFullscreenChanging}
            /> <span class="slider"></span>
          </label>
        </div>
      </div>
      <p class="description-small">
        Start and stay in fullscreen unless you turn it off here.
      </p>
      {#if !isLinux}
        <div class="console-settings">
          <span class="label-text">Enable Background Animation</span>
          <div class="switch-container">
            <label class="switch">
              <input
                type="checkbox"
                checked={isBackgroundAnimationEnabled}
                onchange={handleBackgroundAnimationChange}
              /> <span class="slider"></span>
            </label>
          </div>
        </div>
        <p class="description-small">
          Enable or disable the animated background. Disabling may improve
          performance on low-end devices.
        </p>
      {/if}

      <!-- Card size slider -->
      <div class="slider-row">
        <div class="slider-label">
          <span class="label-text">Card Size</span>
          <span class="value">{Math.round($cardScale * 100)}%</span>
        </div>
        <input
          class="range"
          type="range"
          min="0.75"
          max="1.4"
          step="0.05"
          bind:value={$cardScale}
          aria-label="Card size"
        />
      </div>
      <p class="description-small">
        Adjust how large mod cards render. Smaller cards fit more per row.
      </p>

      <div class="console-settings">
        <span class="label-text">Enable Discord Rich Presence</span>
        <div class="switch-container">
          <label class="switch">
            <input
              type="checkbox"
              checked={isDiscordRpcEnabled}
              onchange={handleDiscordRpcChange}
            /> <span class="slider"></span>
          </label>
        </div>
      </div>
      <p class="description-small">
        Show your Balatro activity in Discord. Displays your current status and
        mod manager usage.
      </p>

      <h3>Privacy</h3>
      <div class="console-settings">
        <span class="label-text">Send anonymous usage data</span>
        <div class="switch-container">
          <label class="switch">
            <input
              type="checkbox"
              checked={isAnalyticsEnabled}
              onchange={handleAnalyticsChange}
            /> <span class="slider"></span>
          </label>
        </div>
      </div>
      <p class="description-small">
        Helps improve BMM. No personal data is collected. Self-hosted on our own
        server. Can be turned off at any time.
      </p>

      <h3>Developer Options</h3>
      <div class="console-settings">
        <span class="label-text">Enable Lovely Console</span>
        <div class="switch-container">
          <label class="switch">
            <input
              type="checkbox"
              checked={isConsoleEnabled}
              onchange={handleConsoleChange}
            /> <span class="slider"></span>
          </label>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .settings-container {
    padding: 0rem 2rem;
    padding-bottom: 2rem;
  }

  h2 {
    font-size: 2.5rem;
    margin-bottom: 2rem;
    color: var(--ui-heading);
  }
  h3 {
    font-size: 1.8rem;
    margin-bottom: 1rem;
    align-self: flex-start;
    color: var(--ui-heading);
  }
  .content {
    flex: 1;
  }
  .reindex-button {
    background: var(--ui-button-green);
    color: var(--ui-text);
    border: none;
    outline: var(--ui-button-green-border) solid 2px;
    border-radius: 4px;
    padding: 0.75rem 1.5rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.2rem;
    cursor: pointer;
    transition: all 0.2s ease;
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .reindex-button:hover {
    background: var(--ui-button-green-hover);
    transform: translateY(-2px);
  }
  .throbber {
    width: 20px;
    height: 20px;
    border: 3px solid var(--ui-spinner);
    border-radius: 50%;
    border-top-color: transparent;
    animation: spin 1s linear infinite;
  }
  .warning {
    color: var(--ui-warning);
    font-size: 1.1rem;
    border-left: 3px solid var(--ui-warning-border);
    padding-left: 0.8rem;
    margin-top: 0.8rem;
    max-width: 600px !important;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .reindex-button:disabled {
    cursor: not-allowed;
    opacity: 0.8;
    transform: none;
  }
  .clear-cache-button {
    background: var(--ui-button-purple);
    color: var(--ui-text);
    border: none;
    outline: var(--ui-button-purple-border) solid 2px;
    border-radius: 4px;
    padding: 0.75rem 1.5rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.2rem;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .clear-cache-button:hover:not(:disabled) {
    background: var(--ui-button-purple-hover);
    transform: translateY(-2px);
  }
  .clear-cache-button:disabled {
    cursor: not-allowed;
    opacity: 0.8;
    transform: none;
  }

  .open-folder-button {
    background: var(--ui-button-forest);
    color: var(--ui-text);
    border: none;
    outline: var(--ui-button-forest-border) solid 2px;
    border-radius: 4px;
    padding: 0.75rem 1.5rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.2rem;
    cursor: pointer;
    transition: all 0.2s ease;
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .open-folder-button:hover {
    background: var(--ui-button-forest-hover);
    transform: translateY(-2px);
  }

  .open-folder-button:active {
    transform: translateY(1px);
  }
  .prefix-input {
    margin-top: 1rem;
    background: var(--ui-input-bg);
    color: var(--ui-text);
    border: 2px solid var(--ui-input-border);
    border-radius: 4px;
    padding: 0.6rem 0.8rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.1rem;
    min-width: 320px;
  }
  .prefix-input::placeholder {
    color: var(--ui-input-placeholder);
  }
  .prefix-button {
    margin-top: 0.6rem;
    background: var(--ui-button-lilac);
    color: var(--ui-text);
    border: none;
    outline: var(--ui-button-lilac-border) solid 2px;
    border-radius: 4px;
    padding: 0.6rem 1.2rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.1rem;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .prefix-button:hover {
    background: var(--ui-button-lilac-hover);
    transform: translateY(-2px);
  }
  .linux-note {
    margin-top: 0.8rem;
    padding: 1rem 1.1rem;
    background: var(--ui-note-bg);
    border: 1px solid var(--ui-note-border);
    border-radius: 4px;
    color: var(--ui-text);
    font-size: 1.1rem;
    max-width: 100%;
    line-height: 1.4;
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
  }
  .linux-note-icon {
    color: var(--ui-note-icon);
    margin-top: 2px;
    display: inline-flex;
  }
  .linux-note-icon :global(svg) {
    display: block;
  }
  .linux-note-content {
    flex: 1;
  }
  .linux-note code {
    font-family: "M6X11", sans-serif;
    background: var(--ui-note-code);
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
  }
  .linux-note-actions {
    margin-top: 0.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .linux-copy-button {
    background: var(--ui-copy-bg);
    color: var(--ui-text);
    border: 2px solid var(--ui-copy-border);
    border-radius: 4px;
    padding: 0.4rem 0.6rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.15rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    transition:
      transform 0.15s ease,
      background 0.15s ease,
      border-color 0.15s ease;
  }
  .linux-copy-button:hover {
    background: var(--ui-copy-hover);
    border-color: var(--ui-copy-hover-border);
    transform: translateY(-1px);
  }
  .linux-copy-button:focus-visible {
    outline: 2px solid var(--ui-note-icon);
    outline-offset: 2px;
  }
  .linux-copy-button:active {
    transform: translateY(0);
    background: var(--ui-copy-active);
    border-color: var(--ui-copy-active-border);
  }
  .linux-what-button {
    background: var(--ui-button-blue);
    color: var(--ui-text);
    border: 2px solid var(--ui-button-blue-border);
    border-radius: 4px;
    padding: 0.4rem 0.6rem;
    font-family: "M6X11", sans-serif;
    font-size: 1.15rem;
    cursor: pointer;
    transition:
      transform 0.15s ease,
      background 0.15s ease,
      border-color 0.15s ease;
  }
  .linux-what-button:hover {
    background: var(--ui-button-blue-hover);
    border-color: var(--ui-button-blue-border-hover);
    transform: translateY(-1px);
  }
  .linux-what-button:focus-visible {
    outline: 2px solid var(--ui-help-focus);
    outline-offset: 2px;
  }
  .linux-what-button:active {
    transform: translateY(0);
    background: var(--ui-button-blue-active);
    border-color: var(--ui-button-blue-active-border);
  }
  .linux-help {
    margin-top: 0.7rem;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1rem;
    width: 100%;
  }
  .linux-help figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .linux-help-button {
    background: transparent;
    border: none;
    padding: 0;
    cursor: zoom-in;
  }
  .linux-help-button:focus-visible {
    outline: 2px solid var(--ui-help-focus);
    outline-offset: 3px;
    border-radius: 6px;
  }
  .linux-help img {
    width: 100%;
    height: auto;
    display: block;
    border: 2px solid var(--ui-help-border);
    border-radius: 6px;
    background: var(--ui-help-bg);
  }
  .linux-help figcaption {
    color: var(--ui-text);
    font-size: 1.2rem;
    opacity: 0.9;
  }
  .image-modal {
    position: fixed;
    inset: 0;
    background: var(--ui-modal-overlay);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 1.5rem;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }
  .image-modal-content {
    position: relative;
    max-width: min(1100px, 92vw);
    max-height: 90vh;
    background: var(--ui-help-bg);
    border: 2px solid var(--ui-help-border);
    border-radius: 10px;
    padding: 1rem 1.2rem 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .image-modal-content img {
    width: 100%;
    height: auto;
    max-height: 75vh;
    object-fit: contain;
    border-radius: 6px;
  }
  .image-modal-content p {
    margin: 0;
    color: var(--ui-text);
    font-size: 1.1rem;
  }
  .image-modal-close {
    position: absolute;
    top: 8px;
    right: 10px;
    background: var(--ui-button-blue);
    color: var(--ui-text);
    border: 2px solid var(--ui-button-blue-border);
    border-radius: 6px;
    width: 32px;
    height: 32px;
    font-size: 1.2rem;
    line-height: 1;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .image-modal-close:hover {
    background: var(--ui-button-blue-hover);
  }
  .description {
    color: var(--ui-text);
    font-size: 1.2rem;
    margin-top: 0.5rem;
    opacity: 0.9;
    max-width: 400px;
    line-height: 1.4;
  } /* Custom Toggle Switch Styles */
  .description-small {
    /* color a bit grayer but still light */
    color: var(--ui-text-muted);
    font-size: 1.1rem;
    margin-top: 0.5rem;
    opacity: 0.9;
    max-width: 400px;
    line-height: 1.4;
  }
  .console-settings {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-top: 1rem;
    font-size: 1.2rem;
    color: var(--ui-text);
  }
  .label-text {
    white-space: nowrap;
  }

  .switch {
    position: relative;
    display: inline-block;
    width: 60px;
    height: 32px;
  }
  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }
  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0; /* Disabled state: red fill and border */
    background-color: var(--ui-switch-off);
    border: 2px solid var(--ui-switch-off-border);
    transition: 0.3s;
    border-radius: 10px;
  }
  .slider:before {
    position: absolute;
    content: "";
    height: 24px;
    width: 24px;
    left: 2px;
    bottom: 2px;
    background-color: var(--ui-switch-thumb);
    /* do a gray outline */
    outline: 2px solid var(--ui-switch-thumb-outline);
    transition: 0.3s;
    border-radius: 5px;
  } /* Enabled state: green fill and border */
  .switch input:checked + .slider {
    background-color: var(--ui-switch-on);
    border: 2px solid var(--ui-switch-on-border);
  }
  .switch input:checked + .slider:before {
    transform: translateX(28px);
  }

  /* Range slider styling */
  .slider-row {
    margin-top: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 420px;
  }
  .slider-label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--ui-text);
    font-size: 1.1rem;
  }
  .slider-label .value {
    color: var(--ui-accent);
  }
  .range {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 28px; /* provide vertical room for thumb */
    background: transparent; /* move visuals to track */
    border: 0;
    box-shadow: none;
  }

  /* Track visuals */
  .range::-webkit-slider-runnable-track {
    height: 12px; /* thicker bar */
    border-radius: 6px;
    background: linear-gradient(
      90deg,
      var(--ui-slider-start),
      var(--ui-slider-end)
    );
    border: 2px solid var(--ui-slider-border);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
  }
  .range::-moz-range-track {
    height: 12px; /* thicker bar */
    border-radius: 6px;
    background: linear-gradient(
      90deg,
      var(--ui-slider-start),
      var(--ui-slider-end)
    );
    border: 2px solid var(--ui-slider-border);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
  }
  .range::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: var(--ui-slider-thumb); /* white thumb */
    border: 2px solid var(--ui-slider-thumb-border); /* subtle gray border */
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.25);
    cursor: pointer;
    position: relative; /* allow offset */
    margin-top: -4px; /* center 20px thumb over 12px track */
  }
  .range::-moz-range-thumb {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: var(--ui-slider-thumb); /* white thumb */
    border: 2px solid var(--ui-slider-thumb-border); /* subtle gray border */
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.25);
    cursor: pointer;
  }
  .range::-webkit-slider-thumb:hover,
  .range::-moz-range-thumb:hover {
    box-shadow: 0 3px 6px rgba(0, 0, 0, 0.3);
  }

  /* Responsive sizing */
  @media (max-width: 1160px) {
    /* Keep slider a comfortable width on smaller screens */
    .slider-row {
      max-width: 300px;
    }
    .slider-label {
      font-size: 1rem;
    }
    .slider-label .value {
      font-size: 0.95rem;
    }
    .range {
      height: 24px;
    }
    .range::-webkit-slider-runnable-track {
      height: 10px;
    }
    .range::-moz-range-track {
      height: 10px;
    }
    .range::-webkit-slider-thumb {
      width: 16px;
      height: 16px;
      margin-top: -3px;
      border-radius: 4px;
    }
    .range::-moz-range-thumb {
      width: 16px;
      height: 16px;
      border-radius: 4px;
    }
  }

  @media (max-width: 1160px) {
    .switch {
      width: 50px;
      height: 24px;
    }
    .slider {
      border-radius: 8px;
    }
    .slider:before {
      height: 16px;
      width: 16px;
      left: 1px;
      bottom: 2px;
      border-radius: 4px;
    }
    .switch input:checked + .slider:before {
      transform: translateX(26px);
    }
  }
  @media (max-width: 1160px) {
    h2 {
      font-size: 2rem;
      transition: all 0.2s ease;
    }
    h3 {
      font-size: 1.5rem;
      transition: all 0.2s ease;
    }
    .reindex-button {
      font-size: 1rem;
      padding: 0.6rem 1.2rem;
    }
    .open-folder-button {
      font-size: 1rem;
      padding: 0.6rem 1.2rem;
    }
    .clear-cache-button {
      font-size: 1rem;
      padding: 0.6rem 1.2rem;
    }
    .description {
      font-size: 1.1rem;
      max-width: 100%;
    }
    .description-small {
      font-size: 1rem;
      max-width: 100%;
    }
  }
</style>
