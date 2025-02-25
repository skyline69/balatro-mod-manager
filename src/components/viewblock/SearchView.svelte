<script lang="ts">
	import { Download, Search, Trash2 } from "lucide-svelte";
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
	import { stripMarkdown, truncateText } from "../../utils/helpers";
	import { currentModView } from "../../stores/modStore";
	import { invoke } from "@tauri-apps/api/core";

	let searchQuery = $state("");
	let searchResults = $state<Mod[]>([]);
	let isSearching = $state(false);
	let searchIndex: any = $state(null);
	let mods = $state<Mod[]>([]);
	let installedMods = $state<InstalledMod[]>([]);
	let mod = $state<Mod | null>(null);

	function handleModClick(mod: Mod) {
		currentModView.set(mod);
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
			});

			await invoke("add_installed_mod", {
				name: mod.title,
				path: installedPath,
				dependencies,
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
		await getAllInstalledMods();
		const status = installedMods.some((m) => m.name === mod.title);
		installationStatus.update((s) => ({ ...s, [mod.title]: status }));
		return status;
	};

	$effect(() => {
		if (mod) {
			isModInstalled(mod);
		}
		mod = $currentModView!;
	});

	onMount(() => {
		// Initialize the search index
		searchIndex = new FlexSearch.Index({
			tokenize: "forward",
			preset: "match",
			cache: true,
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
			isSearching = false;
		}
	}, 300);

	function handleInput() {
		handleSearch();
	}

	/* Later, for CSS
	.tag {
		display: flex;
		align-items: center;
		position: relative;
		gap: 0.2rem;
		padding: 0.15rem 0.3rem;
		background: rgba(0, 0, 0, 0.7);
		border-radius: 4px;
		font-size: 0.9rem;
		color: #f4eee0;
	}
*/
</script>

<div class="search-container">
	<div class="search-bar">
		<form onsubmit={handleSearch}>
			<input
				type="text"
				bind:value={searchQuery}
				oninput={handleInput}
				placeholder="Search mods... (Author or Title)"
				class="search-input"
			/>
			<button type="submit" class="search-button">
				<Search size={20} />
			</button>
		</form>
	</div>

	<div class="results-container">
		{#if isSearching}
			<p class="loading-text">Searching...</p>
		{:else if searchResults.length === 0 && searchQuery.length >= 2}
			<p>No mods found matching "{searchQuery}"</p>
		{:else if searchResults.length > 0}
			{#each searchResults as mod}
				<div
					class="mod-card"
					onclick={() => handleModClick(mod)}
					onkeydown={(e) => e.key === "Enter" && handleModClick(mod)}
					role="button"
					tabindex="0"
					style="--orig-color1: {mod.colors
						.color1}; --orig-color2: {mod.colors.color2};"
				>
					<div class="mod-image">
						<img
							src={mod.image}
							alt={mod.title}
							draggable="false"
						/>
						<div class="tags">
							<!-- <span class="tag updated"> -->
							<!-- 	<Clock size={13} /> -->
							<!-- 	{mod.lastUpdated} -->
							<!-- </span> -->
						</div>
					</div>
					<div class="mod-info">
						<h3>{mod.title}</h3>
						<p>{truncateText(stripMarkdown(mod.description))}</p>
					</div>
					<div class="button-container">
						<button
							class="download-button"
							class:installed={$installationStatus[mod.title]}
							disabled={$installationStatus[mod.title] ||
								$loadingStates[mod.title]}
							onclick={() => installMod(mod)}
						>
							{#if $loadingStates[mod.title]}
								<div class="spinner"></div>
							{:else}
								<Download size={18} />
								{$installationStatus[mod.title]
									? "Installed"
									: "Download"}
							{/if}
						</button>
						{#if $installationStatus[mod.title]}
							<button
								class="delete-button"
								title="Remove Mod"
								onclick={() => uninstallMod(mod)}
							>
								<Trash2 size={18} />
							</button>
						{/if}
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.search-container {
		position: relative;
		width: 75%;
		padding: 1rem;
	}

	::selection {
		background: #ea9600;
		color: #f4eee0;
	}

	.search-bar {
		margin-bottom: 2rem;
	}

	.search-bar form {
		display: flex;
		gap: 0.5rem;
	}

	.search-input {
		flex: 1;
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
	.search-button {
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
	}

	.results-container {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: 1rem;
	}

	.mod-card {
		--bg-color: var(--orig-color1, #4f6367);
		--bg-color-2: var(--orig-color2, #334461);

		display: flex;
		flex-direction: column;
		position: relative;
		border-radius: 8px;
		overflow: hidden;
		border: 2px solid #f4eee0;
		width: 300px;
		height: 330px;
		margin: 0 auto;
		padding: 1rem;
		box-sizing: border-box;
		cursor: pointer;
		background-size: 100% 200%;
		transition: all 0.3s ease;
		background-image: repeating-linear-gradient(
			-45deg,
			var(--bg-color),
			var(--bg-color) 10px,
			var(--bg-color-2) 10px,
			var(--bg-color-2) 20px
		);
	}

	.mod-card:hover {
		animation: stripe-slide-up 1s linear infinite;
		scale: 1.05;
	}

	@keyframes stripe-slide-up {
		0% {
			background-position: 0 0;
		}
		100% {
			background-position: 0 -20px;
		}
	}

	.mod-image {
		position: relative;
		height: 150px;
	}

	.mod-image img {
		width: 100%;
		height: 100%;
		border-radius: 5px;
		object-fit: cover;
	}

	.tags {
		position: absolute;
		top: 7.2rem;
		right: 0.35rem;
		display: flex;
		gap: 0.5rem;
	}

	.mod-info {
		flex: 1;
		padding: 0.5rem;
		position: relative;
		bottom: 1rem;
	}

	.mod-info h3 {
		color: #fdcf51;
		font-size: 1.5rem;
		margin-bottom: 0.2rem;
	}

	.mod-info p {
		color: #f4eee0;
		font-size: 1rem;
		line-height: 1.2;
	}

	.button-container {
		display: flex;
		gap: 0.5rem;
		position: absolute;
		bottom: 1rem;
		left: 1rem;
		width: calc(100% - 2rem);
	}

	.download-button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		padding: 0.75rem;
		background: #56a786;
		color: #f4eee0;
		border: none;
		outline: #459373 solid 2px;
		border-radius: 4px;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.download-button:hover:not(.installed) {
		background: #63b897;
		transform: translateY(-2px);
	}

	.download-button.installed {
		background: #808080;
		outline-color: #666666;
		cursor: not-allowed;
	}
	.download-button:active:not(.installed) {
		transform: translateY(1px);
	}

	.delete-button {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.75rem;
		background: #c14139;
		color: #f4eee0;
		border: none;
		outline: #a13029 solid 2px;
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	@media (max-width: 1160px) {
		.search-container {
			width: 70%;
		}
	}
	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid #f4eee0;
		border-bottom-color: transparent;
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin: 0 auto;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.download-button:disabled {
		opacity: 0.8;
		cursor: not-allowed;
	}
</style>
