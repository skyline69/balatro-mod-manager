<script lang="ts">
	import {
		Download,
		Flame,
		Clock,
		Star,
		Spade,
		Gamepad2,
		LayoutDashboard,
		FolderHeart,
		Trash2,
		Search,
		BookOpen,
	} from "lucide-svelte";
	import ModView from "./ModView.svelte";
	import { tick } from "svelte";
	import { SortOption, currentSort } from "../../stores/modStore";
	import { ArrowUpDown } from "lucide-svelte";

	import {
		currentModView,
		currentCategory,
		uninstallDialogStore,
	} from "../../stores/modStore";
	import type { Mod } from "../../stores/modStore";
	import { Category } from "../../stores/modStore";
	import {
		modsStore,
		installationStatus,
		loadingStates2 as loadingStates,
	} from "../../stores/modStore";
	import type { InstalledMod } from "../../stores/modStore";
	import { open } from "@tauri-apps/plugin-shell";
	import { invoke } from "@tauri-apps/api/core";
	import { stripMarkdown, truncateText } from "../../utils/helpers";
	import SearchView from "./SearchView.svelte";
	import { onMount } from "svelte";
	import { writable } from "svelte/store";

	const loadingDots = writable(0);

	let installedMods: InstalledMod[] = [];

	// Animate the dots
	let dotInterval: number;
	onMount(() => {
		dotInterval = setInterval(() => {
			loadingDots.update((n) => (n + 1) % 4);
		}, 500);

		return () => {
			clearInterval(dotInterval);
		};
	});

	let mods: Mod[] = [];
	let isLoading = true;

	interface DependencyCheck {
		steamodded: boolean;
		talisman: boolean;
	}

	export let handleDependencyCheck: (requirements: DependencyCheck) => void;
	function onDependencyCheck(
		event: CustomEvent<{ steamodded: boolean; talisman: boolean }>,
	) {
		handleDependencyCheck(event.detail);
	}

	export let mod: Mod | undefined;

	async function updateInstallStatus(mod: Mod | undefined) {
		if (!mod) return;
		const status: boolean = await isModInstalled(mod);
		installationStatus.update((s) => ({ ...s, [mod.title]: status }));
	}

	$: {
		if (mod) {
			updateInstallStatus(mod);
		}
	}

	onMount(() => {
		const initialize = async () => {
			if ($modsStore.length === 0) {
				try {
					isLoading = true;
					mods = await fetchModDirectories();
					// Handle installation status updates
				} finally {
					isLoading = false;
				}
			}
			try {
				isLoading = true;
				mods = await fetchModDirectories();
				await Promise.all(
					mods.map(async (mod) => {
						const status = await isModInstalled(mod);
						installationStatus.update((s) => ({
							...s,
							[mod.title]: status,
						}));
					}),
				);

				isLoading = false;
			} catch (error) {
				console.error("Failed to get modloader:", error);
				isLoading = false;
			}
		};

		initialize();

		// Return synchronous cleanup function
		return () => {
			// Cleanup code here if needed
		};
	});

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
				(m) => m.name === mod.title,
			);
			if (!installedMod) return;

			if (isCoreMod) {
				// Get fresh dependencies list
				const dependents = await invoke<string[]>("get_dependents", {
					modName: mod.title,
				});

				// Force UI update before showing dialog
				await tick();

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
		if (!mod?.title || !mod?.downloadURL) return;

		// Collect dependencies first
		const dependencies = [];
		if (mod.requires_steamodded) dependencies.push("Steamodded");
		if (mod.requires_talisman) dependencies.push("Talisman");

		// Existing dependency check logic
		if (mod.requires_steamodded || mod.requires_talisman) {
			const steamoddedInstalled = mod.requires_steamodded
				? await invoke<boolean>("check_mod_installation", {
						modType: "Steamodded",
					})
				: true;
			const talismanInstalled = mod.requires_talisman
				? await invoke<boolean>("check_mod_installation", {
						modType: "Talisman",
					})
				: true;

			if (!steamoddedInstalled || !talismanInstalled) {
				handleDependencyCheck({
					steamodded: mod.requires_steamodded && !steamoddedInstalled,
					talisman: mod.requires_talisman && !talismanInstalled,
				});
				return;
			}
		}

		try {
			loadingStates.update((s) => ({ ...s, [mod.title]: true }));
			let installedPath = await invoke<string>("install_mod", {
				url: mod.downloadURL,
			});
			// if (mod.title.toLowerCase() == "talisman") {
			// 	installedPath = installedPath += "Talisman";
			// }
			//
			const pathExists = await invoke("verify_path_exists", {
				path: installedPath,
			});
			if (!pathExists) {
				throw new Error(
					"Installation failed - files not found at destination",
				);
			}

			// Determine dependencies based on mod type
			let modDependencies = dependencies;
			if (mod.title.toLowerCase() === "steamodded") {
				modDependencies = [];
			} else if (mod.title.toLowerCase() === "talisman") {
				modDependencies = [];
			}

			await invoke("add_installed_mod", {
				name: mod.title,
				path: installedPath,
				dependencies: dependencies,
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
		if (!mod?.title) return false;
		await getAllInstalledMods();
		const status = installedMods.some((m) => m.name === mod.title);
		installationStatus.update((s) => ({ ...s, [mod.title]: status }));
		return status;
	};

	interface ModMeta {
		title: string;
		"requires-steamodded": boolean;
		"requires-talisman": boolean;
		categories: string[];
		author: string;
		repo: string;
		downloadURL?: string;
	}

	const CACHE_DURATION = 15 * 60 * 1000; // 15 minutes
	// const CACHE_DURATION = 5 * 1000; // 5 seconds

	async function saveToCache(mods: Mod[]) {
		await invoke("save_mods_cache", { mods });
	}

	async function getFromCache(): Promise<{
		mods: Mod[];
		timestamp: number;
	} | null> {
		try {
			const cached = await invoke<[Mod[], number] | null>(
				"load_mods_cache",
			);
			if (!cached) return null;

			const [mods, timestamp] = cached;
			return { mods, timestamp };
		} catch (error) {
			console.error("Error loading cache:", error);
			return null;
		}
	}

	async function checkImageExists(imageUrl: string): Promise<string> {
		try {
			const controller = new AbortController();
			const timeoutId = setTimeout(() => controller.abort(), 5000);

			const response = await fetch(imageUrl, {
				method: "HEAD",
				signal: controller.signal,
				// Suppress console errors
				credentials: "omit",
			});

			clearTimeout(timeoutId);
			return response.ok ? imageUrl : "images/cover.jpg";
		} catch {
			return "images/cover.jpg";
		}
	}

	async function getLastUpdated(repoUrl: string): Promise<string> {
		try {
			const [owner, repo] = repoUrl
				.replace("https://github.com/", "")
				.split("/");

			if (!owner || !repo) {
				return "Unknown";
			}

			const response = await fetch(
				`https://api.github.com/repos/${owner}/${repo}/commits/main`,
			);

			if (!response.ok) {
				return "Unknown";
			}

			const data = await response.json();
			if (!data?.commit?.committer?.date) {
				return "Unknown";
			}

			const commitDate = new Date(data.commit.committer.date);
			const currentDate = new Date();
			const diffTime = Math.abs(
				currentDate.getTime() - commitDate.getTime(),
			);
			const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));

			if (diffDays === 0) return "Today";
			if (diffDays === 1) return "Yesterday";
			if (diffDays < 7) return `${diffDays} days ago`;
			if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
			if (diffDays < 365)
				return `${Math.floor(diffDays / 30)} months ago`;
			return `${Math.floor(diffDays / 365)} years ago`;
		} catch (error) {
			console.error("Failed to fetch last commit date:", error);
			return "Unknown";
		}
	}

	async function fetchModDirectories() {
		try {
			isLoading = true;

			// Check cache with proper await
			const cached = await getFromCache();

			if (
				cached &&
				Date.now() - cached.timestamp * 1000 < CACHE_DURATION
			) {
				modsStore.set(cached.mods);
				return cached.mods;
			}

			const response = await fetch(
				"https://api.github.com/repos/skyline69/balatro-mod-index/contents/mods",
			);
			if (!response.ok) {
				throw new Error(`GitHub API returned ${response.status}`);
			}

			const directories = await response.json();

			// Process mods with null filtering
			const mods = (
				await Promise.all(
					directories.map(async (dir: any) => {
						try {
							const [metaResponse, descResponse] =
								await Promise.all([
									fetch(
										`https://raw.githubusercontent.com/skyline69/balatro-mod-index/main/mods/${dir.name}/meta.json`,
									),
									fetch(
										`https://raw.githubusercontent.com/skyline69/balatro-mod-index/main/mods/${dir.name}/description.md`,
									),
								]);

							if (!metaResponse.ok || !descResponse.ok) {
								throw new Error("Failed to fetch mod data");
							}

							const meta: ModMeta = await metaResponse.json();
							const description = await descResponse.text();

							// Handle last updated date
							let lastUpdated = "Unknown";
							try {
								lastUpdated = await getLastUpdated(meta.repo);
							} catch {
								console.log(
									`Failed to fetch last updated for ${meta.title}`,
								);
							}

							// Check image existence
							const imageUrl = await checkImageExists(
								`https://raw.githubusercontent.com/skyline69/balatro-mod-index/main/mods/${dir.name}/thumbnail.jpg`,
							);

							// Convert categories to enum values with validation

							const categories = meta.categories
								.map((cat) => categoryMap[cat])
								.filter((c): c is Category => c !== undefined);

							return {
								title: meta.title,
								description,
								image: imageUrl,
								lastUpdated,
								categories,
								colors: getRandomColorPair(),
								installed: false,
								requires_steamodded:
									meta["requires-steamodded"],
								requires_talisman: meta["requires-talisman"],
								publisher: meta.author,
								repo: meta.repo,
								downloadURL: meta.downloadURL,
							};
						} catch (error) {
							console.error(
								`Failed to process mod ${dir.name}:`,
								error,
							);
							return null;
						}
					}),
				)
			).filter((mod): mod is Mod => mod !== null);

			await saveToCache(mods);
			modsStore.set(mods);
			return mods;
		} catch (error) {
			console.error("Failed to fetch mods:", error);
			return [];
		} finally {
			isLoading = false;
		}
	}
	const categories = [
		{ name: "Installed Mods", icon: Download },
		{ name: "Search", icon: Search },
		{ name: "All Mods", icon: LayoutDashboard },
		{ name: "Content", icon: FolderHeart },
		{ name: "Miscellaneous", icon: BookOpen },
		{ name: "Joker", icon: Flame },
		{ name: "Quality of Life", icon: Star },
		{ name: "Technical", icon: Spade },
		{ name: "Resource Packs", icon: FolderHeart },
		{ name: "API", icon: Gamepad2 },
	];

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

	const categoryMap: Record<string, Category> = {
		Content: Category.Content,
		Joker: Category.Joker,
		"Quality of Life": Category.QualityOfLife,
		Technical: Category.Technical,
		Miscellaneous: Category.Miscellaneous,
		"Resource Packs": Category.ResourcePacks,
		API: Category.API,
	};

	function getRandomColorPair() {
		return colorPairs[Math.floor(Math.random() * colorPairs.length)];
	}

	function handleModClick(mod: Mod) {
		currentModView.set(mod);
	}

	let showSearch: boolean = false;
	$currentCategory = "All Mods";

	$: showSearch = $currentCategory === "Search";

	$: filteredMods = $modsStore.filter((mod) => {
		switch ($currentCategory) {
			case "Content":
				return mod.categories.includes(Category.Content);
			case "Joker":
				return mod.categories.includes(Category.Joker);
			case "Quality of Life":
				return mod.categories.includes(Category.QualityOfLife);
			case "Technical":
				return mod.categories.includes(Category.Technical);
			case "Resource Packs":
				return mod.categories.includes(Category.ResourcePacks);
			case "API":
				return mod.categories.includes(Category.API);
			case "Miscellaneous":
				return mod.categories.includes(Category.Miscellaneous);
			case "Installed Mods":
				return $installationStatus[mod.title];
			default:
				return true;
		}
	});

	function handleCategoryClick(category: string) {
		currentCategory.set(category);
	}

	document.addEventListener("click", (e) => {
		const target = e.target as HTMLElement;
		const anchor = target.closest("a");

		if (anchor && anchor.href.startsWith("https://") && anchor.href) {
			e.preventDefault();
			open(anchor.href);
		}
	});

	function sortMods(mods: Mod[], sortOption: SortOption): Mod[] {
		return [...mods].sort((a, b) => {
			switch (sortOption) {
				case SortOption.NameAsc:
					return a.title.localeCompare(b.title);
				case SortOption.NameDesc:
					return b.title.localeCompare(a.title);
				case SortOption.LastUpdatedAsc:
					return a.lastUpdated.localeCompare(b.lastUpdated);
				case SortOption.LastUpdatedDesc:
					return b.lastUpdated.localeCompare(a.lastUpdated);
				default:
					return 0;
			}
		});
	}

	// Add sort handler
	function handleSortChange(event: Event) {
		const select = event.target as HTMLSelectElement;
		currentSort.set(select.value as SortOption);
	}
	$: sortedAndFilteredMods = sortMods(filteredMods, $currentSort);
</script>

<div class="mods-container">
	<div class="categories">
		{#each categories as category}
			<button
				class:active={$currentCategory === category.name}
				on:click={() => handleCategoryClick(category.name)}
			>
				<svelte:component this={category.icon} size={16} />
				{category.name}
			</button>
		{/each}
	</div>

	<div class="separator"></div>

	{#if isLoading}
		<div class="loading-container">
			<p class="loading-text">Loading mods{".".repeat($loadingDots)}</p>
		</div>
	{:else if showSearch}
		<SearchView />
	{:else}
		<div class="mods-wrapper">
			<div class="sort-controls">
				<div class="sort-wrapper">
					<ArrowUpDown size={16} />
					<select value={$currentSort} on:change={handleSortChange}>
						<option value={SortOption.NameAsc}>Name (A-Z)</option>
						<option value={SortOption.NameDesc}>Name (Z-A)</option>
						<option value={SortOption.LastUpdatedDesc}
							>Latest Updated</option
						>
						<option value={SortOption.LastUpdatedAsc}
							>Oldest Updated</option
						>
					</select>
				</div>
			</div>
			<div class="mods-grid">
				{#each sortedAndFilteredMods as mod}
					<div
						class="mod-card"
						style="--orig-color1: {mod.colors
							.color1}; --orig-color2: {mod.colors.color2};"
						on:click={() => handleModClick(mod)}
						on:keydown={(e) =>
							e.key === "Enter" && handleModClick(mod)}
						role="button"
						tabindex="0"
					>
						<div class="mod-image">
							<img
								src={mod.image}
								alt={mod.title}
								draggable="false"
							/>
							<div class="tags">
								<span class="tag updated">
									<Clock size={13} />
									{mod.lastUpdated}
								</span>
							</div>
						</div>
						<div class="mod-info">
							<h3>{mod.title}</h3>
							<p>
								{truncateText(stripMarkdown(mod.description))}
							</p>
						</div>
						<div class="button-container">
							<button
								class="download-button"
								class:installed={$installationStatus[mod.title]}
								disabled={$installationStatus[mod.title] ||
									$loadingStates[mod.title]}
								on:click|stopPropagation={() => installMod(mod)}
							>
								{#if $loadingStates[mod.title]}
									<div class="spinner"></div>
								{:else}
									<Download size={16} />
									{$installationStatus[mod.title]
										? "Installed"
										: "Download"}
								{/if}
							</button>

							{#if $installationStatus[mod.title]}
								<button
									class="delete-button"
									on:click|stopPropagation={() =>
										uninstallMod(mod)}
								>
									<Trash2 size={16} />
								</button>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>

<ModView mod={$currentModView} on:checkDependencies={onDependencyCheck} />

<style>
	.mods-container {
		display: flex;
		gap: 1rem;
		height: 100%;
	}

	.separator {
		width: 2px;
		background: #f4eee0;
		height: 100%;
	}

	.categories {
		width: 190px;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		overflow-y: auto;

		&::-webkit-scrollbar {
			width: 10px;
		}

		&::-webkit-scrollbar-track {
			background: transparent;
			border-radius: 15px;
		}

		&::-webkit-scrollbar-thumb {
			background: #f4eee0;
			border: 2px solid rgba(193, 65, 57, 0.8);
			border-radius: 15px;
		}
		&::-webkit-scrollbar:horizontal {
			display: none;
		}
		&::-webkit-scrollbar-corner {
			background-color: transparent;
		}

		scrollbar-width: 0;
		scrollbar-color: transparent transparent;
	}

	.categories button {
		text-align: left;
		padding: 1rem 1rem;
		background: #ea9600;
		border: 2px solid #f4eee0;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		border-radius: 6px;
		margin-right: 0.3rem;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.categories button:hover {
		background: #f4eee0;
		color: #393646;
	}

	.categories button.active {
		background: #f4eee0;
		color: #393646;
	}

	.mods-grid {
		height: 95%;
		flex: 1;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: 30px;
		overflow-y: auto;
		padding: 1rem;

		&::-webkit-scrollbar {
			width: 10px;
		}

		&::-webkit-scrollbar-track {
			background: transparent;
			border-radius: 15px;
		}

		&::-webkit-scrollbar-thumb {
			background: #f4eee0;
			border: 2px solid rgba(193, 65, 57, 0.8);
			border-radius: 15px;
		}
		&::-webkit-scrollbar:horizontal {
			display: none;
		}
		&::-webkit-scrollbar-corner {
			background-color: transparent;
		}

		/* scrollbar-width: 0; */
		/* scrollbar-color: transparent transparent; */
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
		/* Remove the duplicate background property and keep only this one */
		background-image: repeating-linear-gradient(
			-45deg,
			var(--bg-color),
			var(--bg-color) 10px,
			var(--bg-color-2) 10px,
			var(--bg-color-2) 20px
		);
	}

	.sort-controls {
		position: fixed;
		top: 2.3rem; /* Increased from 2rem */
		right: 3rem; /* Increased from 2.5rem */
		z-index: 1000;
		margin: 0;
		background: transparent;
		transform: translateY(0); /* Reset any transforms */
	}

	.sort-wrapper {
		background: #ea9600;
		border: 2px solid #f4eee0;
		padding: 0.5rem;
		border-radius: 6px;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		transition: all 0.2s ease;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
	}

	.mods-wrapper {
		flex: 1;
		position: relative;
		overflow: hidden;
	}

	.sort-wrapper :global(svg) {
		color: #f4eee0;
	}

	select {
		background: #ea9600;
		color: #f4eee0;
		border: none;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		padding: 0.25rem 1.5rem 0.25rem 0.5rem;
		border-radius: 4px;
		cursor: pointer;
		-webkit-appearance: none;
		-moz-appearance: none;
		appearance: none;
		background-image: url("data:image/svg+xml;charset=US-ASCII,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%22292.4%22%20height%3D%22292.4%22%3E%3Cpath%20fill%3D%22%23F4EEE0%22%20d%3D%22M287%2069.4a17.6%2017.6%200%200%200-13-5.4H18.4c-5%200-9.3%201.8-12.9%205.4A17.6%2017.6%200%200%200%200%2082.2c0%205%201.8%209.3%205.4%2012.9l128%20127.9c3.6%203.6%207.8%205.4%2012.8%205.4s9.2-1.8%2012.8-5.4L287%2095c3.5-3.5%205.4-7.8%205.4-12.8%200-5-1.9-9.2-5.4-12.8z%22%2F%3E%3C%2Fsvg%3E");
		background-repeat: no-repeat;
		background-position: right 0.5em top 50%;
		background-size: 0.65em auto;
	}

	select:hover {
		background-color: #f0a620;
	}

	select:focus {
		outline: none;
		box-shadow: 0 0 0 2px #f4eee0;
	}

	select option {
		background: #ea9600;
		color: #f4eee0;
		padding: 0.5rem;
	}

	.sort-wrapper:hover {
		transform: translateY(-1px);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
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
		border: 2px solid #f4eee0;
	}

	.tags {
		position: absolute;
		top: 7.2rem; /* Adjusted top position */
		right: 0.35rem; /* Adjusted right position */
		display: flex;
		gap: 0.5rem;
	}
	.tag :global(svg) {
		position: relative;
		top: -1px; /* Adds subtle upward adjustment to the icons */
	}

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
	.download-button {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		width: calc(100% - 2rem); /* Account for parent padding */
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
		position: absolute;
		bottom: 1rem;
		left: 1rem;

		&.installed {
			background: #808080;
			outline-color: #666666;
			cursor: not-allowed;
		}

		&.installed:hover {
			background: #808080;
			transform: none;
		}
	}

	.download-button:hover {
		background: #74cca8;
		transform: translateY(-2px);
	}

	.download-button:active {
		transform: translateY(0);
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
		position: static;
		bottom: auto;
		left: auto;
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

	.delete-button:hover {
		background: #d4524a;
		transform: translateY(-2px);
	}

	.delete-button:active {
		transform: translateY(0);
	}

	.loading-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		flex: 1;
	}

	.loading-text {
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 1.5rem;
		min-width: 150px;
	}

	.spinner {
		width: 13px;
		height: 13px;
		border: 2px solid #f4eee0;
		border-bottom-color: transparent;
		border-radius: 50%;
		animation: spin 1s linear infinite;
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

	/* Add these media queries at the end of the style section */
	@media (max-width: 768px) {
		.mods-container {
			flex-direction: column;
			gap: 0.5rem;
		}

		.categories {
			width: 100%;
			flex-direction: row;
			overflow-x: auto;
			padding-bottom: 0.5rem;
			height: auto;
		}

		.categories button {
			flex-shrink: 0;
			padding: 0.75rem;
			font-size: 0.9rem;
		}

		.separator {
			width: 100%;
			height: 2px;
			margin: 0.5rem 0;
		}

		.sort-controls {
			position: sticky;
			top: 0;
			right: auto;
			left: 50%;
			transform: translateX(-50%);
			width: fit-content;
			margin: 0.5rem auto;
			z-index: 1000;
		}

		.mods-grid {
			padding-top: 70px;
			grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
			gap: 20px;
			padding: 0.5rem;
		}

		.mod-card {
			width: 100%;
			max-width: 280px;
			height: 300px;
		}

		.mod-image {
			height: 130px;
		}

		.mod-info h3 {
			font-size: 1.3rem;
		}

		.mod-info p {
			font-size: 0.9rem;
		}
	}

	@media (max-width: 480px) {
		.sort-controls {
			top: 0.5rem;
		}

		.sort-wrapper {
			padding: 0.4rem;
		}

		select {
			font-size: 0.9rem;
			padding-right: 1.2rem;
		}

		.mods-grid {
			grid-template-columns: 1fr;
			padding: 0.5rem;
		}

		.mod-card {
			height: auto;
			min-height: 280px;
		}

		.button-container {
			position: relative;
			bottom: auto;
			left: auto;
			margin-top: 0.5rem;
		}

		.download-button {
			position: relative;
			bottom: auto;
			left: auto;
		}

		.tags {
			top: 6rem;
		}
	}
</style>
