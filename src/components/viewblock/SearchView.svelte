<script lang="ts">
	import type { InstalledMod, Mod } from "../../stores/modStore";
	import { onMount } from "svelte";
	import {
		installationStatus,
		modsStore,
		loadingStates2 as loadingStates,
		uninstallDialogStore,
	} from "../../stores/modStore";
	import { debounce } from "lodash";
	import FlexSearch from "flexsearch";
	import { currentModView } from "../../stores/modStore";
	import { invoke } from "@tauri-apps/api/core";
	import { fade } from "svelte/transition";
	import ModCard from "./ModCard.svelte";

	let searchQuery = $state("");
	let searchResults = $state<Mod[]>([]);
	let isSearching = $state(false);
	let searchIndex: any = $state(null);
	let mods = $state<Mod[]>([]);
	let installedMods = $state<InstalledMod[]>([]);
	let mod = $state<Mod | null>(null);
	let searchInput: HTMLInputElement;

	function handleModClick(mod: Mod) {
		currentModView.set(mod);
	}

	function focusSearchInput() {
		if (searchInput) {
			searchInput.focus();
		}
	}

	const { onCheckDependencies } = $props<{
		onCheckDependencies?: (event: {
			steamodded: boolean;
			talisman: boolean;
		}) => void;
	}>();

	const getAllInstalledMods = async () => {
		try {
			const installed: InstalledMod[] = await invoke(
				"get_installed_mods_from_db",
			);
			// fill the installed mods Array
			installedMods = installed.map((mod) => {
				return {
					name: mod.name,
					path: mod.path,
					// collection_hash: mod.collection_hash,
				};
			});
		} catch (error) {
			console.error("Failed to get installed mods:", error);
		}
	};

	const uninstallMod = async (mod: Mod) => {
		const isCoreMod = ["steamodded", "talisman"].includes(
			mod.title.toLowerCase(),
		);

		try {
			await getAllInstalledMods();
			const installedMod = installedMods.find(
				(m) => m.name.toLowerCase() === mod.title.toLowerCase(),
			);

			if (!installedMod) {
				console.error("Mod not found in installed mods");
				return;
			}

			if (isCoreMod) {
				// Get dependents
				const dependents = await invoke<string[]>("get_dependents", {
					modName: mod.title,
				});

				// Always show dialog for core mods, even if no dependents
				uninstallDialogStore.set({
					show: true,
					modName: mod.title,
					modPath: installedMod.path,
					dependents,
				});
			} else {
				// Immediate uninstall for normal mods
				await invoke("remove_installed_mod", {
					name: mod.title,
					path: installedMod.path,
				});
				installationStatus.update((s) => ({
					...s,
					[mod.title]: false,
				}));
			}
		} catch (error) {
			console.error("Uninstall failed:", error);
		}
	};

	const installMod = async (mod: Mod) => {
		// Check dependencies first before doing anything else
		if (mod.requires_steamodded || mod.requires_talisman) {
			// Check Steamodded if required
			const steamoddedInstalled = mod.requires_steamodded
				? await invoke<boolean>("check_mod_installation", {
						modType: "Steamodded",
					})
				: true;

			// Check Talisman if required
			const talismanInstalled = mod.requires_talisman
				? await invoke<boolean>("check_mod_installation", {
						modType: "Talisman",
					})
				: true;

			// If any dependency is missing, show the RequiresPopup
			if (
				(mod.requires_steamodded && !steamoddedInstalled) ||
				(mod.requires_talisman && !talismanInstalled)
			) {
				// Call the handler with the appropriate requirements
				onCheckDependencies?.({
					steamodded: mod.requires_steamodded && !steamoddedInstalled,
					talisman: mod.requires_talisman && !talismanInstalled,
				});
				return; // Stop installation
			}
		}

		// Create dependencies list for the database
		const dependencies = [];
		if (mod.requires_steamodded) dependencies.push("Steamodded");
		if (mod.requires_talisman) dependencies.push("Talisman");

		try {
			loadingStates.update((s) => ({ ...s, [mod.title]: true }));
			const installedPath = await invoke<string>("install_mod", {
				url: mod.downloadURL,
				folderName: mod.folderName || mod.title.replace(/\s+/g, ""),
			});

			await invoke("add_installed_mod", {
				name: mod.title,
				path: installedPath,
				dependencies,
				currentVersion: mod.version || "",
			});

			await getAllInstalledMods();
			installationStatus.update((s) => ({ ...s, [mod.title]: true }));
		} catch (error) {
			console.error("Failed to install mod:", error);
		} finally {
			loadingStates.update((s) => ({ ...s, [mod.title]: false }));
		}
	};

	const isModInstalled = async (mod: Mod) => {
		if (!mod) return false;

		await getAllInstalledMods();
		const status = installedMods.some((m) => m.name === mod.title);

		// Only update the store if the status has changed
		const currentStatus = $installationStatus[mod.title];
		if (currentStatus !== status) {
			installationStatus.update((s) => ({ ...s, [mod.title]: status }));
		}

		return status;
	};

	let prevMod: Mod | null = null;

	$effect(() => {
		const newMod = $currentModView;

		// Only proceed if newMod is different from the previous mod
		if (newMod && (!prevMod || newMod.title !== prevMod.title)) {
			prevMod = newMod;
			mod = newMod;

			// Move the installation check outside of the reactive context
			setTimeout(() => {
				isModInstalled(newMod);
			}, 0);
		}
	});

	onMount(() => {
		// Initialize the search index
		searchIndex = new FlexSearch.Index({
			tokenize: "forward",
			preset: "match",
			cache: true,
		});

		$effect(() => {
			if (searchInput) {
				searchInput.focus();
			}
		});

		// Subscribe to mods store
		return modsStore.subscribe((currentMods) => {
			mods = currentMods;
			if (mods.length > 0) {
				// Instead of clear(), recreate the index
				searchIndex = new FlexSearch.Index({
					tokenize: "forward",
					preset: "match",
					cache: true,
				});

				mods.forEach((mod, idx) => {
					const searchText =
						`${mod.title} ${mod.publisher}`.toLowerCase();
					searchIndex.add(idx, searchText);
				});
			}
		});
	});

	const handleSearch = debounce(() => {
		if (!searchIndex || searchQuery.length < 2) {
			searchResults = [];
			showSpinner = false;
			return;
		}

		isSearching = true;

		try {
			const searchTerm = searchQuery.toLowerCase();
			const results = searchIndex.search(searchTerm);

			searchResults = results.map((idx: number) => mods[idx]);
		} catch (error) {
			console.error("Search failed:", error);
			searchResults = [];
		} finally {
			showSpinner = false;
			isSearching = false;
		}
	}, 300);

	let showSpinner = $state(false);

	function handleInput() {
		showSpinner = true;
		handleSearch();
	}
</script>

<div class="search-container">
	<div class="search-bar">
		<form onsubmit={handleSearch}>
			<input
				bind:this={searchInput}
				type="text"
				bind:value={searchQuery}
				oninput={handleInput}
				placeholder="Search mods... (Author or Title)"
				class="search-input"
			/>
			<!-- <button type="submit" class="search-button">
				<Search size={20} />
			</button> -->
		</form>

		{#if showSpinner}
			<!-- svelte-ignore element_invalid_self_closing_tag -->
			<div transition:fade={{ duration: 100 }} class="search-spinner" />
		{/if}
	</div>

	<div class="results-scroll-container default-scrollbar">
		<div class="results-container">
			{#if isSearching}
				<p transition:fade={{ duration: 100 }} class="resulting-text">
					Searching...
				</p>
			{:else if searchResults.length === 0 && searchQuery.length >= 2}
				<p transition:fade={{ duration: 100 }} class="resulting-text">
					No mods found matching "{searchQuery}"
				</p>
			{:else if searchResults.length > 0}
				<div
					transition:fade={{ duration: 100 }}
					class="results-wrapper"
				>
					{#each searchResults as mod}
						<ModCard
							{mod}
							oninstallclick={installMod}
							onuninstallclick={uninstallMod}
							onmodclick={handleModClick}
						/>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.search-container {
		position: relative;
		/* 192px being the width of the catagories + seperator */
		width: calc(100% - 192px);
		padding: 0 1rem;
	}

	::selection {
		background: #ea9600;
		color: #f4eee0;
	}

	.search-bar {
		height: 3rem;
		/* accounting for the padding (2rem) & scroll container's scrollbar (0.625rem/10px)*/
		width: calc(100% - 2.625rem);
		position: absolute;
		top: 1rem;
		z-index: 100;
	}

	.search-spinner {
		display: block;
		position: absolute;
		top: 25%;
		left: calc(100% - 2.5rem);
		width: 1rem;
		height: 1rem;
		z-index: 10;
		animation: spin infinite 1s linear;
		border-radius: 9999px;
		border: 2px solid #f4eee0;
		border-right: 2px solid transparent;
	}

	.search-bar form {
		display: flex;
		gap: 0.5rem;
		width: 100%;
	}

	.search-input {
		/* 2rem just for some spacing from the scrollbar */
		width: calc(100% - 2rem);
		padding: 0.75rem;
		border: 2px solid #f4eee0;
		border-radius: 6px;
		background-color: #393646;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
	}
	.search-input:focus {
		outline: none;
		border-color: #ea9600;
		transition: border-color 0.2s ease;
	}
	/* legacy search button code */
	/* .search-button {
		padding: 0.75rem 1rem;
		background: #ea9600;
		border: 2px solid #f4eee0;
		border-radius: 6px;
		color: #f4eee0;
		cursor: pointer;
		display: flex;
		align-items: center;
		transition: all 0.2s ease;
	}

	.search-button:hover {
		background: #f4eee0;
		color: #393646;
	}

	.search-button:active {
		transform: scale(0.95);
		padding: 0.75rem 0.95rem;
	} */

	.resulting-text {
		position: absolute;
	}

	.results-container {
		padding: 1rem;
		padding-top: 5rem;
	}

	.results-wrapper {
		width: 100%;
		height: 100%;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: 1rem;
	}

	.results-scroll-container {
		overflow-y: auto;
		height: 100%;
	}

	@media (max-width: 1160px) {
		.results-container {
			padding: 1rem;
			padding-top: 5rem;
		}
	}
</style>
