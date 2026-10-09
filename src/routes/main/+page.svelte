<script lang="ts">
  // Lazy-load ShaderBackground only when enabled
  import type { Component } from "svelte";
  let ShaderBackgroundComp = $state<Component | null>(null);
  import About from "../../components/viewblock/About.svelte";
  import LaunchButton from "../../components/LaunchButton.svelte";
  import Mods from "../../components/viewblock/Mods.svelte";
  import SubmitMod from "../../components/viewblock/SubmitMod.svelte";
  import Settings from "../../components/viewblock/Settings.svelte";
  import RequiresPopup from "../../components/RequiresPopup.svelte";
  import SecurityPopup from "../../components/SecurityPopup.svelte";
  import LovelyMissingPopup from "../../components/LovelyMissingPopup.svelte";
  import CollectionImportPopup from "../../components/CollectionImportPopup.svelte";
  import type { DependencyCheck, InstalledMod } from "../../stores/modStore";
  import {
    currentModView,
    currentCategory,
    modsStore,
  } from "../../stores/modStore";
  import { backgroundEnabled } from "../../stores/modStore";
  import { selectedModStore, dependentsStore } from "../../stores/modStore";
  import { currentPage, paginationWindow } from "../../stores/modStore";
  import {
    installationStatus,
    showWarningPopup,
    requiresPopupStore,
    securityPopupStore,
  } from "../../stores/modStore";
  import { invoke } from "@tauri-apps/api/core";
  import { fetchCachedMods, forceRefreshCache } from "../../stores/modCache";
  import { addMessage } from "$lib/stores";
  import UninstallDialog from "../../components/UninstallDialog.svelte";
  import { onMount } from "svelte";
  import { lovelyPopupStore } from "../../stores/modStore";
  import { cardScale, darkMode } from "../../stores/ui";
  import { get } from "svelte/store";
  import ReportIssue from "../../components/ReportIssue.svelte";
  import CollectionPicker from "../../components/CollectionPicker.svelte";
  import BackupsPanel from "../../components/BackupsPanel.svelte";
  import CreateBackupModal from "../../components/CreateBackupModal.svelte";
  import RestoreBackupPopup from "../../components/RestoreBackupPopup.svelte";
  import DeleteBackupPopup from "../../components/DeleteBackupPopup.svelte";
  import { fade } from "svelte/transition";
  import { isLinuxPlatform } from "$lib/platform";
  import { backupsStore } from "../../stores/backups";

  let currentSection = $state("mods");
  let isLinux = $state(false);
  let hasMounted = $state(false);
  let appVersion = $state("");

  let requiresPopupDismissedAt = 0;
  let wasRequiresPopupVisible = false;

  let storedDownloadAction: (() => Promise<void>) | null = $state(null);
  let originalDownloadAction: (() => Promise<void>) | null = $state(null);

  // Function to check if security warning needs to be shown
  async function checkSecurityAcknowledgment(): Promise<boolean> {
    try {
      const isAcknowledged = await invoke<boolean>(
        "is_security_warning_acknowledged",
      );
      return isAcknowledged;
    } catch (error) {
      console.error("Failed to check security acknowledgment:", error);
      return false; // If there's an error, show the popup anyway
    }
  }

  // Modified to include security check
  async function handleDependencyCheck(
    requirements: DependencyCheck,
    downloadAction?: () => Promise<void>,
  ) {
    if (Date.now() - requiresPopupDismissedAt < 200) {
      return;
    }
    modRequirements = requirements;
    if (downloadAction) {
      originalDownloadAction = downloadAction;

      // Check if we need to show the security popup first
      const isSecurityAcknowledged = await checkSecurityAcknowledgment();

      if (!isSecurityAcknowledged) {
        // Store the action but don't execute it yet - show security popup first
        storedDownloadAction = null;
        securityPopupStore.set({
          visible: true,
          onAcknowledge: handleSecurityAcknowledge,
          onCancel: handleSecurityCancel,
        });
      } else {
        // Security already acknowledged, proceed with dependency check
        storedDownloadAction = downloadAction;
        requiresPopupStore.set({
          visible: true,
          requiresSteamodded: requirements.steamodded,
          requiresTalisman: requirements.talisman,
          onProceed: handleProceedDownload,
          onDependencyClick: handleDependencyClick,
        });
      }
    } else {
      console.warn("handleDependencyCheck called without a download action");
      storedDownloadAction = null;
      originalDownloadAction = null;
    }
  }

  // Handle security acknowledgment
  async function handleSecurityAcknowledge() {
    securityPopupStore.update((s) => ({ ...s, visible: false }));

    // Now proceed with dependency check if there was an action
    if (originalDownloadAction) {
      storedDownloadAction = originalDownloadAction;
      requiresPopupStore.set({
        visible: true,
        requiresSteamodded: modRequirements.steamodded,
        requiresTalisman: modRequirements.talisman,
        onProceed: handleProceedDownload,
        onDependencyClick: handleDependencyClick,
      });
    }
  }

  // Handle security cancellation
  function handleSecurityCancel() {
    securityPopupStore.update((s) => ({ ...s, visible: false }));
    storedDownloadAction = null;
    originalDownloadAction = null;
  }

  function nextPage() {
    if ($currentPage < $paginationWindow.totalPages) {
      currentPage.update((n) => n + 1);
    }
  }

  function previousPage() {
    if ($currentPage > 1) {
      currentPage.update((n) => n - 1);
    }
  }

  function goToPage(page: number) {
    if (page < 1 || page > $paginationWindow.totalPages) return;
    currentPage.set(page);
  }

  function handleProceedDownload() {
    if (storedDownloadAction) {
      storedDownloadAction().catch((error) => {
        console.error("Error during download action execution:", error);
        showError(error);
      });
    } else {
      console.warn(
        "Proceed action requested, but no download action was stored.",
      );
    }
    storedDownloadAction = null; // Clear the stored action
    originalDownloadAction = null; // Clear the original action too
  }

  let contentElement: HTMLDivElement;

  let showUninstallDialog = $state(false);
  const selectedMod = $derived($selectedModStore);

  async function handleRefresh() {
    // Force-refresh cache so removal reflects immediately
    await forceRefreshCache();
    const installedMods: InstalledMod[] = await fetchCachedMods();
    installationStatus.set(
      Object.fromEntries(
        installedMods.map((mod: InstalledMod) => [mod.name, true]),
      ),
    );
  }

  function showError(error: unknown) {
    addMessage(
      `Uninstall failed: ${error instanceof Error ? error.message : String(error)}`,
      "error",
    );
  }

  function onError(event: { detail: unknown }) {
    showError(event.detail);
  }

  function onUninstalled(_event: {
    detail: { modName: string; success: boolean; action: string };
  }) {
    handleRefresh();
  }

  let modRequirements = $state({
    steamodded: false,
    talisman: false,
  });

  function handleDependencyClick(dependency: string) {
    // Find the mod in the store
    const mods = get(modsStore);
    const foundMod = mods.find(
      (m) => m.title.toLowerCase() === dependency.toLowerCase(),
    );

    // If found, open it in the mod view
    if (foundMod) {
      currentModView.set(foundMod);
    } else {
      console.warn(`Dependency mod not found: ${dependency}`);
    }
  }

  function handleRequestUninstall(
    event: CustomEvent<{ mod: InstalledMod; dependents: string[] }>,
  ) {
    selectedModStore.set(event.detail.mod);
    dependentsStore.set(event.detail.dependents);
    showUninstallDialog = true;
  }

  interface AppInitData {
    version: string;
    existing_installation: string | null;
    security_acknowledged: boolean;
    lovely_installed: boolean;
    lovely_update_available: string | null;
    launch_mode: string;
  }

  onMount(async () => {
    isLinux = await isLinuxPlatform();
    hasMounted = true;
    handleRefresh();

    // Fetch all init data in a single batched IPC call
    try {
      const initData = await invoke<AppInitData>("get_app_init_data");
      appVersion = initData.version;

      // Check if we need to show the security popup
      if (!initData.security_acknowledged) {
        securityPopupStore.set({
          visible: true,
          onAcknowledge: handleSecurityAcknowledge,
          onCancel: handleSecurityCancel,
        });
      }

      // Check Lovely status
      if (!initData.lovely_installed) {
        // Not installed: show install prompt
        lovelyPopupStore.set({ visible: true });
      } else if (initData.lovely_update_available) {
        // Lovely is installed but update available
        showWarningPopup.set({
          visible: true,
          message: `An update for Lovely (v${initData.lovely_update_available}) is available. Do you want to update?`,
          onConfirm: async () => {
            try {
              const updated = await invoke<string>("update_lovely_to_latest");
              addMessage(`Lovely updated to v${updated}`, "success");
            } catch (e) {
              addMessage(
                `Failed to update Lovely: ${e instanceof Error ? e.message : String(e)}`,
                "error",
              );
            }
            showWarningPopup.update((p) => ({ ...p, visible: false }));
          },
          onCancel: () => {
            showWarningPopup.update((p) => ({ ...p, visible: false }));
          },
        });
      }
    } catch (error) {
      console.error("Failed to load init data:", error);
      // Fallback to individual calls if batch fails
      try {
        appVersion = await invoke<string>("get_app_version");
      } catch (_) {
        appVersion = "";
      }
    }
  });

  $effect(() => {
    if (wasRequiresPopupVisible && !$requiresPopupStore.visible) {
      storedDownloadAction = null;
      originalDownloadAction = null;
      requiresPopupDismissedAt = Date.now();
    }
    wasRequiresPopupVisible = $requiresPopupStore.visible;
  });

  $effect(() => {
    if (!hasMounted || isLinux) {
      return;
    }

    if ($backgroundEnabled && !ShaderBackgroundComp) {
      import("../../components/ShaderBackground.svelte")
        .then((m) => {
          ShaderBackgroundComp = m.default;
        })
        .catch(() => {});
    }
  });
</script>

<!-- Background shader is dynamically imported below when enabled -->

{#if $backgroundEnabled && ShaderBackgroundComp && !isLinux}
  <ShaderBackgroundComp darkMode={$darkMode} />
{/if}

<div class="main-page">
  <header>
    <div class="header-content">
      <h1>Balatro Mod Manager</h1>
      <LaunchButton />
    </div>
    <nav>
      <button
        class:active={currentSection === "mods"}
        onclick={() => (currentSection = "mods")}
      >
        Mods
      </button>
      <button
        class:active={currentSection === "backups"}
        onclick={() => {
          currentSection = "backups";
          backupsStore.load();
        }}
      >
        Backups
      </button>
      <button
        class:active={currentSection === "submit"}
        onclick={() => (currentSection = "submit")}
      >
        Submit Mod
      </button>
      <button
        class:active={currentSection === "settings"}
        onclick={() => (currentSection = "settings")}
      >
        Settings
      </button>
      <button
        class:active={currentSection === "about"}
        onclick={() => (currentSection = "about")}
      >
        About
      </button>
    </nav>
  </header>

  <div
    class="content"
    class:modal-open={!!$currentModView && currentSection == "mods"}
    bind:this={contentElement}
    style="--card-scale: {$cardScale}"
  >
    <!-- All sections stay mounted for smooth transitions -->
    <div class="section-wrapper" class:active={currentSection === "mods"}>
      <Mods mod={null} {handleDependencyCheck} />
    </div>

    <div class="section-wrapper" class:active={currentSection === "backups"}>
      <BackupsPanel />
    </div>

    <div class="section-wrapper" class:active={currentSection === "submit"}>
      <SubmitMod />
    </div>

    <div class="section-wrapper" class:active={currentSection === "settings"}>
      <Settings />
    </div>

    <div class="section-wrapper" class:active={currentSection === "about"}>
      <About />
    </div>
  </div>

  {#if currentSection === "mods" && !$currentModView && $currentCategory !== "Search" && $currentCategory !== "Collections" && $currentCategory !== "Installed Mods" && $paginationWindow.totalPages > 1}
    <div
      class="pagination-footer"
      in:fade={{ duration: 150 }}
      out:fade={{ duration: 120 }}
    >
      <div class="pagination-controls">
        <button onclick={previousPage} disabled={$currentPage === 1}>
          Previous
        </button>
        {#each Array(Math.min($paginationWindow.maxVisiblePages, $paginationWindow.totalPages)) as _, i (i)}
          {#if $paginationWindow.startPage + i <= $paginationWindow.totalPages}
            <button
              class:active={$currentPage === $paginationWindow.startPage + i}
              onclick={() => goToPage($paginationWindow.startPage + i)}
            >
              {$paginationWindow.startPage + i}
            </button>
          {/if}
        {/each}
        <button
          onclick={nextPage}
          disabled={$currentPage === $paginationWindow.totalPages}
        >
          Next
        </button>
      </div>
    </div>
  {/if}

  <RequiresPopup />

  <SecurityPopup />

  <UninstallDialog
    bind:show={showUninstallDialog}
    modName={selectedMod?.name ?? ""}
    modPath={selectedMod?.path ?? ""}
    bind:dependents={$dependentsStore}
    {onUninstalled}
    {onError}
  />

  {#if appVersion}<div class="version-text">v0.4.1</div>{/if}
</div>

<LovelyMissingPopup />
<CollectionPicker />
<CollectionImportPopup />
<ReportIssue />
<CreateBackupModal />
<RestoreBackupPopup />
<DeleteBackupPopup />

<style>
  .main-page {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    padding: 2rem;
    box-sizing: border-box;
    background: transparent;
  }
  header {
    margin-bottom: -1rem;
  }

  h1 {
    color: var(--ui-text);
    font-size: 3rem;
    margin-bottom: 2rem;
    font-family: "M6X11", sans-serif;
    text-shadow:
      -2px -2px 0 #000,
      2px -2px 0 #000,
      -2px 2px 0 #000,
      2px 2px 0 #000;
  }

  nav {
    display: flex;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  button {
    background: transparent;
    border: 2px solid var(--ui-text);
    color: var(--ui-text);
    padding: 0.7rem 1.4rem;
    border-radius: 8px;
    font-family: "M6X11", sans-serif;
    font-size: 1.2rem;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  button:hover,
  button.active {
    background: var(--ui-mod-chip-active-bg);
    color: var(--ui-mod-chip-active-text);
  }

  .content {
    flex: 1;
    background: var(--ui-danger-overlay);
    border-radius: 5px;
    backdrop-filter: blur(10px);
    margin-bottom: 2rem;
    outline: 2px solid var(--ui-danger-overlay-border-strong);
    /* overflow-y: auto; Enable vertical scrolling */
    overflow: hidden;
    max-height: calc(100vh - 12rem);
    min-height: 0;
  }

  .content.modal-open {
    overflow: hidden !important;
    /* scrollbar-gutter: stable; */
  }

  /* Add scrollbar width variable for consistency */
  :global(:root) {
    --scrollbar-width: 10px;
  }

  .content.modal-open {
    /* padding-right: var(--scrollbar-width); */
    padding-right: 0;
  }

  .section-wrapper {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    opacity: 0;
    visibility: hidden;
    transition:
      opacity 0.2s ease,
      visibility 0.2s ease;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .section-wrapper::-webkit-scrollbar {
    width: 10px;
  }

  .section-wrapper::-webkit-scrollbar-track {
    background: transparent;
    border-radius: 15px;
  }

  .section-wrapper::-webkit-scrollbar-thumb {
    background: var(--ui-scroll-thumb);
    border: 2px solid var(--ui-scroll-thumb-border);
    border-radius: 15px;
  }

  .section-wrapper.active {
    opacity: 1;
    visibility: visible;
  }

  .content {
    position: relative;
  }

  .version-text {
    position: fixed;
    bottom: 1rem;
    right: 1rem;
    color: var(--ui-text);
    font-family: "M6X11", sans-serif;
    text-shadow:
      -1px -1px 0 #000,
      1px -1px 0 #000,
      -1px 1px 0 #000,
      1px 1px 0 #000;
  }
  .header-content {
    position: relative;
    margin-bottom: 2rem;
  }

  .pagination-footer {
    position: fixed;
    left: 50%;
    bottom: 1.1rem;
    transform: translateX(-50%);
    display: flex;
    justify-content: center;
    z-index: 1400;
  }

  .pagination-controls {
    display: flex;
    gap: 0.4rem;
    padding: 0;
    background: transparent;
    border: none;
    box-shadow: none;
  }

  .pagination-controls button {
    padding: 0.45rem 0.9rem;
    background: var(--ui-mod-chip-bg);
    border: 1px solid var(--ui-mod-chip-border);
    color: var(--ui-mod-chip-text);
    font-family: "M6X11", sans-serif;
    font-size: 0.9rem;
    cursor: pointer;
    border-radius: 3px;
    transition: all 0.2s ease;
  }

  .pagination-controls button:hover:not(:disabled) {
    background: var(--ui-mod-chip-active-bg);
    color: var(--ui-mod-chip-active-text);
  }

  .pagination-controls button.active {
    background: var(--ui-mod-chip-active-bg);
    color: var(--ui-mod-chip-active-text);
  }

  .pagination-controls button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  header {
    margin-bottom: -1rem;
  }

  :global([data-platform="linux"]) .content {
    backdrop-filter: none;
    background: var(--ui-danger-overlay-strong);
  }

  @media (max-width: 1160px) {
    button {
      padding: 0.6rem 1.2rem;
      border-radius: 8px;
      font-family: "M6X11", sans-serif;
      font-size: 0.9rem;
      cursor: pointer;
      transition: all 0.2s ease;
    }
  }
</style>
