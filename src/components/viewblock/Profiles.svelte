<script lang="ts">
  import { onMount } from "svelte";
  import {
    profilesStore,
    activeProfileId,
    installedDirsStore,
    refreshProfiles,
    refreshInstalledDirs,
    createProfile,
    deleteProfile,
    renameProfile,
    saveProfileMods,
    activateProfile,
    deactivateProfiles,
    type ModProfile,
    type SimpleModDir,
  } from "../../stores/profiles";
  import { addMessage } from "$lib/stores";
  import { Trash2 } from "lucide-svelte";

  let profiles: ModProfile[] = $state([]);
  let dirs: SimpleModDir[] = $state([]);
  let selectedId: number | null = $state(null);
  let selectedDirs: Set<string> = $state(new Set()); // folder names
  let creating = $state(false);
  let newName = $state("");
  let renamingId: number | null = $state(null);
  let renameText = $state("");
  const activeId = $derived($activeProfileId);
  const NAME_MAX: number = 24;

  // UI helpers
  let modFilter = $state("");
  const filteredDirs = $derived(
    modFilter.trim()
      ? dirs.filter((d) =>
          `${d.name} ${d.dir_name}`
            .toLowerCase()
            .includes(modFilter.trim().toLowerCase()),
        )
      : dirs,
  );
  const selectedCount = $derived(selectedDirs.size);
  const allVisibleSelected = $derived(
    filteredDirs.length > 0 &&
      filteredDirs.every((d) => selectedDirs.has(d.dir_name)),
  );

  $effect(() => {
    const unsubP = profilesStore.subscribe((v) => {
      profiles = v;
      // If selection is empty, pick first
      if (selectedId === null && v.length > 0) {
        selectProfile(v[0].id);
      } else if (selectedId !== null) {
        // keep selectedMods synced if this profile changed
        const p = v.find((x) => x.id === selectedId);
        if (p) selectedDirs = new Set(p.mods);
      }
    });
    const unsubD = installedDirsStore.subscribe((v) => (dirs = v));
    return () => {
      unsubP();
      unsubD();
    };
  });

  onMount(async () => {
    try {
      await Promise.all([refreshProfiles(), refreshInstalledDirs()]);
    } catch (e) {
      console.error("Failed to load profiles:", e);
    }
  });

  function selectProfile(id: number) {
    selectedId = id;
    const p = profiles.find((x) => x.id === id);
    selectedDirs = new Set(p ? p.mods : []);
    renamingId = null;
  }

  function isValidName(name: string): boolean {
    const t = name.trim();
    return t.length > 0 && t.length <= NAME_MAX;
  }

  async function handleCreate() {
    if (!isValidName(newName)) {
      addMessage(`Name must be 1–${NAME_MAX} characters.`, "error");
      return;
    }
    try {
      const id = await createProfile(newName.trim().slice(0, NAME_MAX));
      newName = "";
      creating = false;
      selectProfile(id);
    } catch (e) {
      addMessage(`Failed to create profile: ${String(e)}`, "error");
    }
  }

  async function handleDelete(id: number) {
    try {
      await deleteProfile(id);
      if (selectedId === id) selectedId = null;
    } catch (e) {
      addMessage(`Delete failed: ${String(e)}`, "error");
    }
  }

  function toggleSelection(dirName: string) {
    if (selectedDirs.has(dirName)) selectedDirs.delete(dirName);
    else selectedDirs.add(dirName);
    // force reactive update
    selectedDirs = new Set(selectedDirs);
  }

  function selectAllVisible() {
    for (const d of filteredDirs) selectedDirs.add(d.dir_name);
    selectedDirs = new Set(selectedDirs);
  }

  function clearAllVisible() {
    for (const d of filteredDirs) selectedDirs.delete(d.dir_name);
    selectedDirs = new Set(selectedDirs);
  }

  async function handleSaveMods() {
    if (selectedId === null) return;
    try {
      await saveProfileMods(selectedId, Array.from(selectedDirs));
      addMessage("Profile updated", "success");
    } catch (e) {
      addMessage(`Failed to save: ${String(e)}`, "error");
    }
  }

  async function handleRename(id: number) {
    if (!isValidName(renameText)) {
      renamingId = null;
      if (renameText.trim().length > NAME_MAX) {
        addMessage(`Name must be ≤ ${NAME_MAX} characters.`, "error");
      }
      return;
    }
    try {
      await renameProfile(id, renameText.trim().slice(0, NAME_MAX));
      renamingId = null;
    } catch (e) {
      addMessage(`Rename failed: ${String(e)}`, "error");
    }
  }

  async function handleActivate(id: number) {
    try {
      await activateProfile(id);
      addMessage("Profile activated", "success");
    } catch (e) {
      addMessage(`Activate failed: ${String(e)}`, "error");
    }
  }

  async function handleDeactivate() {
    try {
      await deactivateProfiles();
      addMessage("Profiles disabled (all mods enabled)", "success");
    } catch (e) {
      addMessage(`Deactivate failed: ${String(e)}`, "error");
    }
  }

  function handleRevert() {
    if (selectedId === null) return;
    const p = profiles.find((x) => x.id === selectedId);
    selectedDirs = new Set(p ? p.mods : []);
  }

  // Color pairs (same palette as mod cards)
  const colorPairs = [
    { color1: "#4f6367", color2: "#425556" },
    { color1: "#AA778D", color2: "#906577" },
    { color1: "#A2615E", color2: "#89534F" },
    { color1: "#A48447", color2: "#8B703C" },
    { color1: "#4F7869", color2: "#436659" },
    { color1: "#728DBF", color2: "#6177A3" },
    { color1: "#5D5E8F", color2: "#4F4F78" },
    { color1: "#796E9E", color2: "#655D86" },
    { color1: "#64825D", color2: "#556E4E" },
    { color1: "#86A367", color2: "#728A57" },
    { color1: "#748C8A", color2: "#627775" },
  ];

  function hashString(s: string): number {
    let h = 2166136261 >>> 0;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return h >>> 0;
  }

  function colorFor(name: string) {
    const idx = hashString(name) % colorPairs.length;
    return colorPairs[idx];
  }
</script>

<div class="profiles-root">
  <aside class="profiles-list">
    <div class="list default-scrollbar">
      {#each profiles as p (p.id)}
        <div class="profile-row">
          <button class="row-main" class:active={p.id === selectedId} onclick={() => selectProfile(p.id)}>
            <span class="name" title={p.name}>{p.name}</span>
            {#if activeId === p.id}
              <span class="badge">Active</span>
            {/if}
          </button>
          <div class="row-actions">
            <button
              class="trash-button"
              title="Delete profile"
              aria-label={`Delete ${p.name}`}
              onclick={() => handleDelete(p.id)}
            >
              <Trash2 size={18} />
            </button>
          </div>
        </div>
      {/each}
    </div>

    <div class="create-box">
      {#if creating}
        <input
          placeholder="Profile name"
          bind:value={newName}
          onkeydown={(e) => e.key === 'Enter' && handleCreate()}
          maxlength={NAME_MAX}
        />
        <div class="row-actions">
          <button class="create-btn" onclick={handleCreate} disabled={!isValidName(newName)}>Create</button>
          <button class="cancel-btn" onclick={() => { creating = false; newName = ''; }}>Cancel</button>
        </div>
      {:else}
        <button class="new-profile-btn" onclick={() => (creating = true)}>+ New Profile</button>
      {/if}
    </div>
  </aside>

  <section class="profile-detail">
    {#if selectedId !== null}
      <div class="detail-header">
        <div class="header-left">
          {#if renamingId === selectedId}
            <input
              class="title-input"
              bind:value={renameText}
              onkeydown={(e) => e.key === 'Enter' && handleRename(selectedId!)}
              maxlength={NAME_MAX}
            />
            <button class="small rename-save" onclick={() => handleRename(selectedId!)} disabled={!isValidName(renameText)}>Save</button>
            <button class="small rename-cancel" onclick={() => (renamingId = null)}>Cancel</button>
          {:else}
            {@const cp = profiles.find((x) => x.id === selectedId)}
            {#if cp}
              <span class="title" title={cp.name}>{cp.name}</span>
              {#if activeId === selectedId}
                <span class="badge">Active</span>
              {/if}
              <button class="small ghost" title="Rename" onclick={() => { renamingId = selectedId; renameText = cp.name; }}>Rename</button>
            {:else}
              <span class="title">Profile</span>
            {/if}
          {/if}
        </div>
        <div class="header-center">
          <span class="meta">{selectedCount}/{dirs.length} enabled</span>
        </div>
        <div class="header-right">
          {#if activeId === selectedId}
            <button class="deactivate" onclick={handleDeactivate}>Deactivate</button>
          {:else}
            <button class="activate" onclick={() => handleActivate(selectedId!)}>Activate</button>
          {/if}
          <button class="ghost" onclick={handleRevert} title="Revert unsaved changes">Revert</button>
          <button class="save" onclick={handleSaveMods}>Save</button>
        </div>
      </div>

      <div class="toolbar">
        <input
          class="filter-input"
          placeholder="Filter mods (name or folder)"
          bind:value={modFilter}
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') modFilter = ''; }}
        />
        <div class="toolbar-spacer" aria-hidden="true"></div>
        <div class="toolbar-actions">
          {#if allVisibleSelected}
            <button class="ghost" onclick={clearAllVisible}>Clear visible</button>
          {:else}
            <button class="ghost" onclick={selectAllVisible}>Select visible</button>
          {/if}
        </div>
      </div>

      <div class="mods-list default-scrollbar">
        {#each filteredDirs as d (d.path)}
          {@const colors = colorFor(d.dir_name)}
          <div
            class="profile-mod-item"
            role="button"
            tabindex="0"
            onclick={() => toggleSelection(d.dir_name)}
            onkeydown={(e) => e.key === 'Enter' && toggleSelection(d.dir_name)}
            style="--bg1: {colors.color1}; --bg2: {colors.color2};"
            title={d.path}
          >
            <span class="mod-title" title={d.name}>{d.name}</span>
            <input
              class="mod-check"
              type="checkbox"
              checked={selectedDirs.has(d.dir_name)}
              onclick={(e) => { e.stopPropagation(); toggleSelection(d.dir_name); }}
              aria-label={`Toggle ${d.name}`}
            />
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty">Create or select a profile to begin.</div>
    {/if}
  </section>
</div>

<style>
  /* Text selection theme for this view */
  ::selection { background: #fdcf51; color: #393646; }
  ::-moz-selection { background: #fdcf51; color: #393646; }
  :root { --sidebar-w: clamp(220px, 24vw, 320px); }
  .profiles-root { display: grid; grid-template-columns: var(--sidebar-w) 1fr; gap: 1rem; height: 100%; overflow: hidden; }
  .profiles-list { background: transparent; border: none; border-radius: 0; padding: 1rem .75rem .75rem 0; display:flex; flex-direction:column; border-right: 2px solid #f4eee0; height: 100%; min-height: 0; position: relative; }
  .list { flex:1; overflow:auto; gap:.35rem; display:flex; flex-direction:column; min-height: 0; padding-right: .25rem; padding-bottom: 4.5rem; }
  .list .profile-row:first-child { margin-top: .4rem; }
  .profile-row { display:flex; align-items:center; gap:.5rem; }
  /* Row container stays neutral; selection styles live on .row-main */
  .row-main { flex:1; display:flex; align-items:center; justify-content:space-between; background:rgba(244,238,224,0.06); border:2px solid #f4eee0; color:#f4eee0; padding:.6rem .8rem; text-align:left; cursor:pointer; position: relative; z-index: 0; border-radius: 8px; transition: transform .15s ease, background .2s ease, border-color .2s ease, box-shadow .2s ease; font-size: 1.05rem; font-family:"M6X11", sans-serif; min-width: 0; }
  .row-main .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row-main.active { border-color:#fdcf51; background: rgba(253, 207, 81, 0.12); box-shadow: 0 2px 0 rgba(253, 207, 81, 0.25) inset; }
  .row-main:hover { transform: translateY(-2px); background: rgba(244,238,224,0.12); }
  .row-actions { display:flex; gap:.35rem; position: relative; z-index: 1; }
  .row-actions button, .create-box button { background:transparent; border:2px solid #f4eee0; color:#f4eee0; border-radius:8px; padding:.44rem .8rem; font-family:"M6X11", sans-serif; cursor:pointer; font-size: 1.05rem; }
  .create-box input { width:100%; box-sizing: border-box; padding:.35rem .5rem; border:2px solid #c88000; border-radius:6px; background:transparent; color:#f4eee0; font-family:"M6X11", sans-serif; font-size: 1.15rem; }
  .create-box input::placeholder { color: rgba(244, 238, 224, 0.8); }
  .create-box input:focus { outline: none; border-color: #fdcf51; box-shadow: 0 0 0 2px rgba(253, 207, 81, 0.18); }
  .trash-button { display:flex; align-items:center; justify-content:center; min-width:42px; height:42px; padding:8px; background:#c14139; color:#f4eee0; border:none; outline:#a13029 solid 2px; border-radius:4px; cursor:pointer; transition: all .2s ease; font-family:"M6X11", sans-serif; }
  .trash-button:hover { background:#d4524a; transform: translateY(-2px); }
  .trash-button:active { transform: translateY(1px); }
  .row-actions button[disabled], .create-box button[disabled], .small[disabled] { opacity: .5; cursor: not-allowed; transform: none; }
  .badge { margin-left:.5rem; background:#00a2ff; color:#f4eee0; border-radius:6px; padding:.12rem .5rem; font-size:.9rem; }
  .create-box { margin-top:.5rem; display:flex; flex-direction:column; gap:.4rem; position: sticky; bottom: .75rem; padding: .6rem; background: #ea9600; border: 2px solid #f4eee0; border-radius: 10px; z-index: 3; box-shadow: 0 -2px 0 rgba(0,0,0,.22), 0 2px 6px rgba(0,0,0,.18) inset; }
  .new-profile-btn {
    background: #fdcf51; /* Balatro gold */
    color: #393646;
    border: none;
    outline: none;
    border-radius: 8px;
    padding: .55rem .9rem;
    width: 100%;
    font-family: "M6X11", sans-serif;
    font-size: 1.1rem;
    cursor: pointer;
    transition: all .2s ease;
    box-shadow: inset 0 0 10px rgba(0,0,0,.25);
  }
  .new-profile-btn:hover { background: #f0a620; transform: translateY(-2px); }
  .new-profile-btn:active { transform: translateY(0); }
  .new-profile-btn:focus-visible { outline: 2px dashed #f4eee0; outline-offset: 2px; }

  .profile-detail { background: transparent; border: none; border-radius: 0; padding: 1rem 0 .75rem .75rem; display:flex; flex-direction:column; height: 100%; min-height: 0; min-width: 0; }
  .detail-header { display:grid; grid-template-columns: 1fr auto auto; align-items:center; margin-bottom:.4rem; gap:.6rem; position: sticky; top: 0; z-index: 2; font-size: 1.05rem; background: rgba(0,0,0,0.08); padding:.5rem .5rem; border-radius: 10px; border: 2px solid #f4eee0; }
  .header-left, .header-center, .header-right { display:flex; align-items:center; gap:.5rem; }
  .title { font-family:"M6X11", sans-serif; font-size:1.25rem; max-width: 28rem; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .title-input { padding:.35rem .5rem; border:2px solid #f4eee0; border-radius:6px; background:transparent; color:#f4eee0; font-family:"M6X11", sans-serif; min-width: 14rem; font-size: 1.2rem; }
  .title-input:focus { outline: none; border-color:#fdcf51; box-shadow: 0 0 0 2px rgba(253,207,81,0.18); }
  .small { font-family:"M6X11", sans-serif; font-size:1rem; padding:.35rem .6rem; border:2px solid #f4eee0; background:transparent; color:#f4eee0; border-radius:8px; cursor:pointer; }
  .ghost { background: transparent; opacity: .95; }
  .meta { font-family:"M6X11", sans-serif; opacity:.95; }
  .toolbar { display:grid; grid-template-columns: 1fr 16px auto; column-gap:0; row-gap:.35rem; align-items:center; margin: .35rem 0 .55rem; }
  .toolbar-spacer { width: 16px; height: 1px; }
  .filter-input { width:100%; min-width: 0; padding:.4rem .6rem; border:2px solid #f4eee0; border-radius:8px; background:transparent; color:#f4eee0; font-family:"M6X11", sans-serif; font-size: 1.05rem; }
  .filter-input:focus { outline: none; border-color:#fdcf51; box-shadow: 0 0 0 2px rgba(253,207,81,0.18); }
  .toolbar-actions { display:flex; gap:.5rem; justify-self: end; white-space: nowrap; }
  /* Responsive grid for mod items */
  .mods-list { display:grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap:.55rem; overflow-y:auto; overflow-x:hidden; padding:.25rem .25rem 1.5rem 0; flex: 1; min-height: 0; position: relative; z-index: 0; }
  .profile-mod-item {
    --bg1: #4f6367;
    --bg2: #334461;
    display:flex; align-items:center; gap:.75rem; padding:.5rem .7rem; border:2px solid #f4eee0; border-radius:8px; color:#f4eee0; box-sizing: border-box; max-width: 100%; width: 100%;
    /* Tile-based stripes for seamless looping */
    background-image: linear-gradient(
      135deg,
      var(--bg1) 25%,
      var(--bg2) 25% 50%,
      var(--bg1) 50% 75%,
      var(--bg2) 75% 100%
    );
    background-size: 18px 18px;
    background-position: 0 0;
    will-change: background-position;
    transition: transform .2s ease, box-shadow .2s ease;
    cursor: pointer;
  }
  .profile-mod-item:hover { transform: translateY(-2px); animation: stripe-tiling 1.1s linear infinite; box-shadow: 0 2px 6px rgba(0,0,0,.25); }
  @keyframes stripe-tiling { 0% { background-position: 0 0; } 100% { background-position: 0 18px; } }
  .mod-title { flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-family:"M6X11", sans-serif; font-size:1.2rem; padding-left: 2px; }
  /* Custom checkbox themed to Balatro */
  .mod-check {
    appearance: none;
    -webkit-appearance: none;
    width: 22px; height: 22px;
    border: 2px solid #f4eee0;
    border-radius: 6px;
    background: #393646;
    position: relative;
    cursor: pointer;
    transition: transform .12s ease, background .12s ease, box-shadow .12s ease;
    box-shadow: inset 0 0 6px rgba(0,0,0,.25);
  }
  .mod-check:hover { transform: translateY(-1px) scale(1.03); }
  .mod-check:focus-visible { outline: 2px dashed #f4eee0; outline-offset: 2px; }
  .mod-check:checked { background: #00a2ff; box-shadow: inset 0 0 6px rgba(0,0,0,.25); }
  .mod-check::after {
    content: "";
    position: absolute;
    left: 6px; top: 3px;
    width: 5px; height: 10px;
    border: 2px solid #f4eee0;
    border-top: 0; border-left: 0;
    transform: rotate(45deg) scale(0);
    transform-origin: center;
    transition: transform .12s ease;
  }
  .mod-check:checked::after { transform: rotate(45deg) scale(1); }
  /* Header action buttons */
  .detail-header .header-right button {
    border: 2px solid #f4eee0;
    color: #f4eee0;
    border-radius: 8px;
    padding: .44rem .8rem;
    font-family: "M6X11", sans-serif;
    cursor: pointer;
    font-size: 1.05rem;
    transition: transform .12s ease, background .15s ease, box-shadow .15s ease;
  }
  .activate { background:#00a2ff; border-color:#334461; }
  .activate:hover { background: #0088ff; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  .deactivate { background:#c14139; border-color:#a13029; }
  .deactivate:hover { background:#d4524a; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  .save { background:#27ae60; color:#f4eee0; border-color:#219653; }
  .save:hover { background:#2ecc71; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  /* Revert as gold-outlined ghost */
  .detail-header .header-right .ghost { background: transparent; color:#fdcf51; border-color:#fdcf51; }
  .detail-header .header-right .ghost:hover { background: rgba(253,207,81,0.15); transform: translateY(-2px); }
  .detail-header .header-right .ghost:active { transform: translateY(0); }
  /* Rename Save/Cancel hover/active */
  .detail-header .header-left .rename-save { background:#27ae60; border:2px solid #219653; color:#f4eee0; }
  .detail-header .header-left .rename-save:hover:not([disabled]) { background:#2ecc71; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  .detail-header .header-left .rename-save:active:not([disabled]) { transform: translateY(0); }
  .detail-header .header-left .rename-cancel { background:#7f8c8d; border:2px solid #636e72; color:#f4eee0; }
  .detail-header .header-left .rename-cancel:hover { background:#95a5a6; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  .detail-header .header-left .rename-cancel:active { transform: translateY(0); }

  /* Chips for Select/Clear visible */
  .toolbar .ghost { background: transparent; color:#f4eee0; border:2px solid #f4eee0; border-radius: 9999px; padding: .35rem .75rem; margin-left: 1rem; font-family:"M6X11", sans-serif; font-size: 0.98rem; cursor: pointer; }
  .toolbar .ghost:hover { background: rgba(244,238,224,0.12); transform: translateY(-1px); }
  .empty { color:#f4eee0; opacity:.9; padding:1rem; }
  /* Specific styles for Create/Cancel in the footer */
  .create-box .create-btn { background:#27ae60; border-color:#219653; color:#f4eee0; }
  .create-box .create-btn:hover:not([disabled]) { background:#2ecc71; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  .create-box .create-btn:active:not([disabled]) { transform: translateY(0); }
  .create-box .cancel-btn { background:#7f8c8d; border-color:#636e72; color:#f4eee0; }
  .create-box .cancel-btn:hover { background:#95a5a6; transform: translateY(-2px); box-shadow: 0 2px 4px rgba(0,0,0,.2); }
  .create-box .cancel-btn:active { transform: translateY(0); }

  @media (max-width: 1160px) {
    .profiles-root { grid-template-columns: 1fr; height: auto; overflow: visible; }
    .detail-header { grid-template-columns: 1fr; }
    .header-right { justify-content: flex-start; flex-wrap: wrap; }
  }

  @media (max-width: 1280px) {
    :root { --sidebar-w: clamp(200px, 22vw, 280px); }
    .mods-list { grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); }
  }

  @media (max-width: 980px) {
    .profiles-list { border-right: none; border-bottom: 2px solid #f4eee0; padding: .75rem 0; height: auto; max-height: 40vh; overflow: auto; }
    .profile-detail { padding: .75rem 0 0 0; height: auto; }
    .toolbar { grid-template-columns: 1fr; }
    .toolbar-spacer { display: none; }
    .toolbar-actions { justify-self: start; }
    .mods-list { grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); }
  }

  @media (max-width: 720px) {
    .detail-header .header-right button { padding: .4rem .6rem; font-size: .95rem; }
  }
</style>
