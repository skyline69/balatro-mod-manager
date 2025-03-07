<script lang="ts">
	import { fade } from "svelte/transition";
	import { cubicOut } from "svelte/easing";
	import {
		Download,
		Trash2,
		User,
		ArrowLeft,
		Github,
		X,
	} from "lucide-svelte";
	import { onMount, onDestroy } from "svelte";
	import { open } from "@tauri-apps/plugin-shell";
	import {
		currentModView,
		installationStatus,
		loadingStates2 as loadingStates,
		uninstallDialogStore,
		currentCategory, // Add this import
	} from "../../stores/modStore";
	import type { InstalledMod, Mod } from "../../stores/modStore";
	import { marked } from "marked";
	import { invoke } from "@tauri-apps/api/core";
	import { cachedVersions } from "../../stores/modStore";
	import { modsStore } from "../../stores/modStore";
	import { untrack } from "svelte";

	const VERSION_CACHE_DURATION = 60 * 60 * 1000;

	interface Props {
		mod: Mod;
		onCheckDependencies?: (event: {
			steamodded: boolean;
			talisman: boolean;
		}) => void;
	}

	const { mod, onCheckDependencies }: Props = $props();
	const isDefaultCover = (imageUrl: string) => imageUrl.includes("cover.jpg");
	function handleAuxClick(event: MouseEvent) {
		if (event.button === 3) {
			event.preventDefault();
			handleBack();
		}
	}

	let installedMods: InstalledMod[] = [];
	let steamoddedVersions = $state<string[]>([]);
	let talismanVersions = $state<string[]>([]);
	let selectedVersion = $state("newest");
	let loadingVersions = $state(false);
	let renderedDescription = $state("");

	let versionLoadStarted = false;
	let prevModTitle = "";
	let hasCheckedInstallation = false;

	let modsArray: Mod[] = [];
	modsStore.subscribe((m) => (modsArray = m));

	let skipHistoryUpdate = false;
	let description: HTMLDivElement;
	// Links gets pushed down the array as another gets added. The oldest link is the last one in the array.
	let history: internalModLinkData[] = $state([]);

	const linkCache = new Map<string, internalModLinkData>();

	let modView: HTMLDivElement;

	interface internalModLinkData {
		isMod: boolean;
		modName: string;
	}

	function isInternalModLink(url: string): internalModLinkData {
		// Quickly check common non-mod paths first
		if (!url || !url.includes("github.com")) {
			return { isMod: false, modName: "" };
		}

		// Exclude specific paths that are not mod repositories
		if (
			url.match(/\.(txt|lua|json|md|png|jpg|jpeg|gif|mp3|ogg|wav)$/) ||
			url.includes("/blob/") ||
			url.includes("/tree/") ||
			url.includes("/wiki") ||
			url.includes("/actions") ||
			url.includes("/issues") ||
			url.includes("/pulls") ||
			url.includes("/commits") ||
			url.includes("/releases") ||
			url.includes("/archive") ||
			url.includes("/compare") ||
			url.includes("/security") ||
			url.includes("/projects")
		) {
			return { isMod: false, modName: "" };
		}

		// Common patterns for mod links
		const githubModPattern1 =
			/github\.com\/([^\/]+)\/([^\/\?#]+)(?:$|\?|\#)/;
		const githubModPattern2 =
			/github\.com\/([^\/]+)\/([^\/\?#]+)(?:\/|\/tree\/|\/blob\/)/;

		// Check if URL matches any pattern
		let match =
			url.match(githubModPattern1) || url.match(githubModPattern2);

		if (match && match[2]) {
			// Repository name from URL
			const repoName = match[2].toLowerCase();

			// Get mods from the store - avoid subscribers in functions that run during rendering
			let modsArray: Mod[] = [];
			const unsubscribe = modsStore.subscribe((m) => (modsArray = m));
			unsubscribe(); // Important: unsubscribe immediately to prevent memory leaks

			// Find matching mod
			const foundMod = modsArray.find((mod) => {
				// Direct match
				if (mod.title.toLowerCase() === repoName) {
					return true;
				}

				// Match on repo URL
				if (mod.repo && mod.repo.toLowerCase().includes(repoName)) {
					return true;
				}

				// Match with spaces replaced
				const titleDashes = mod.title
					.toLowerCase()
					.replace(/\s+/g, "-");
				const titleUnderscores = mod.title
					.toLowerCase()
					.replace(/\s+/g, "_");
				const titleNoSpaces = mod.title
					.toLowerCase()
					.replace(/\s+/g, "");

				return (
					repoName === titleDashes ||
					repoName === titleUnderscores ||
					repoName === titleNoSpaces
				);
			});

			if (foundMod) {
				return { isMod: true, modName: foundMod.title };
			}
		}

		return { isMod: false, modName: "" };
	}

	async function loadSteamoddedVersions() {
		if (loadingVersions) return;
		try {
			const cached = await invoke<[string[], number]>(
				"load_versions_cache",
				{ modType: "steamodded" },
			);
			if (cached) {
				const [cachedVers, cachedTs] = cached;
				if (Date.now() - cachedTs * 1000 < VERSION_CACHE_DURATION) {
					steamoddedVersions = cachedVers;
					selectedVersion = "newest";
					if (steamoddedVersions.length > 0) {
						selectedVersion = steamoddedVersions[0];
					}
					cachedVersions.update((c) => ({
						...c,
						steamodded: cachedVers,
					}));
					return;
				}
			}
		} catch (e) {
			console.log("Version cache check failed:", e);
		}
		loadingVersions = true;
		try {
			const versions: string[] = await invoke("get_steamodded_versions");
			steamoddedVersions = versions;
			selectedVersion = "newest";

			if (versions.length > 0) {
				selectedVersion = versions[0];
			}

			cachedVersions.update((c) => ({ ...c, steamodded: versions }));
			await invoke("save_versions_cache", {
				modType: "steamodded",
				versions,
			});
		} catch (e) {
			console.error("Failed to load Steamodded versions:", e);
			steamoddedVersions = [];
		} finally {
			loadingVersions = false;
		}
	}

	async function loadTalismanVersions() {
		if (loadingVersions) return;
		try {
			const cached = await invoke<[string[], number]>(
				"load_versions_cache",
				{ modType: "talisman" },
			);
			if (cached) {
				const [cachedVers, cachedTs] = cached;
				if (Date.now() - cachedTs * 1000 < VERSION_CACHE_DURATION) {
					talismanVersions = cachedVers;
					if (cachedVers.length > 0) {
						selectedVersion = cachedVers[0];
					}
					cachedVersions.update((c) => ({
						...c,
						talisman: cachedVers,
					}));
					return;
				}
			}
		} catch (e) {
			console.log("Version cache check failed:", e);
		}
		loadingVersions = true;
		try {
			const versions: string[] = await invoke("get_talisman_versions");
			talismanVersions = versions;
			if (versions.length > 0) {
				selectedVersion = versions[0];
			}
			cachedVersions.update((c) => ({ ...c, talisman: versions }));
			await invoke("save_versions_cache", {
				modType: "talisman",
				versions,
			});
		} catch (e) {
			console.error("Failed to load Talisman versions:", e);
			talismanVersions = [];
		} finally {
			loadingVersions = false;
		}
	}

	const getAllInstalledMods = async () => {
		try {
			const installed: InstalledMod[] = await invoke(
				"get_installed_mods_from_db",
			);
			installedMods = installed.map((m) => ({
				name: m.name,
				path: m.path,
			}));
		} catch (e) {
			console.error("Failed to get installed mods:", e);
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

			if (!installedMod) return;

			if (isCoreMod) {
				// Get dependents
				const dependents = await invoke<string[]>("get_dependents", {
					modName: mod.title,
				});

				// Always show the dialog for core mods
				uninstallDialogStore.set({
					show: true,
					modName: mod.title,
					modPath: installedMod.path,
					dependents,
				});
			} else {
				await invoke("remove_installed_mod", {
					name: mod.title,
					path: installedMod.path,
				});
				installationStatus.update((s) => ({
					...s,
					[mod.title]: false,
				}));
			}
		} catch (e) {
			console.error("Failed to uninstall mod:", e);
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

		// Build dependencies list for the database
		const dependencies = [];
		if (mod.requires_steamodded) dependencies.push("Steamodded");
		if (mod.requires_talisman) dependencies.push("Talisman");

		try {
			loadingStates.update((s) => ({ ...s, [mod.title]: true }));

			if (mod.title.toLowerCase() === "steamodded") {
				let installedPath;
				if (selectedVersion === "newest") {
					installedPath = await invoke<string>("install_mod", {
						url: mod.downloadURL,
						folderName:
							mod.folderName || mod.title.replace(/\s+/g, ""),
					});
				} else {
					installedPath = await invoke<string>(
						"install_steamodded_version",
						{ version: selectedVersion },
					);
				}
				const pathExists = await invoke("verify_path_exists", {
					path: installedPath,
				});
				if (!pathExists)
					throw new Error(
						"Installation failed - files not found at destination",
					);
				await invoke("add_installed_mod", {
					name: mod.title,
					path: installedPath,
					dependencies,
				});
				await getAllInstalledMods();
				installationStatus.update((s) => ({ ...s, [mod.title]: true }));
			} else if (mod.title.toLowerCase() === "talisman") {
				let installedPath;
				if (selectedVersion === "newest") {
					installedPath = await invoke<string>("install_mod", {
						url: mod.downloadURL,
						folderName:
							mod.folderName || mod.title.replace(/\s+/g, ""),
					});
				} else {
					installedPath = await invoke<string>(
						"install_talisman_version",
						{ version: selectedVersion },
					);
				}
				const pathExists = await invoke("verify_path_exists", {
					path: installedPath,
				});
				if (!pathExists)
					throw new Error(
						"Installation failed - files not found at destination",
					);
				await invoke("add_installed_mod", {
					name: mod.title,
					path: installedPath,
					dependencies: [],
				});
				await getAllInstalledMods();
				installationStatus.update((s) => ({ ...s, [mod.title]: true }));
			} else {
				const installedPath = await invoke<string>("install_mod", {
					url: mod.downloadURL,
					folderName: mod.folderName || mod.title.replace(/\s+/g, ""),
				});
				await invoke("add_installed_mod", {
					name: mod.title,
					path: installedPath,
					dependencies,
				});
				await getAllInstalledMods();
				installationStatus.update((s) => ({ ...s, [mod.title]: true }));
			}
		} catch (e) {
			console.error("Failed to install mod:", e);
		} finally {
			loadingStates.update((s) => ({ ...s, [mod.title]: false }));
		}
	};

	function handleMarkdownClick(event: MouseEvent | KeyboardEvent) {
		const anchor = (event.target as HTMLElement).closest("a");
		if (!anchor || !anchor.href) return;

		// Check if this is an internal mod link using the data attribute
		const internalModName = anchor.getAttribute("data-internal-mod");

		if (internalModName) {
			event.preventDefault();
			event.stopPropagation(); // This is crucial

			let modsArray: Mod[] = [];
			modsStore.subscribe((m) => (modsArray = m))();

			const targetMod = modsArray.find(
				(m) => m.title === internalModName,
			);
			if (targetMod) {
				currentModView.set(targetMod);
			}
		} else if (anchor.href.startsWith("http")) {
			// External link
			event.preventDefault();
			open(anchor.href).catch((e) =>
				console.error("Failed to open link:", e),
			);
		}
	}

	/**
	 * Sets the current mod view by title
	 * @param title
	 * @returns success
	 */
	function setModViewByTitle(title: string): boolean {
		const targetMod = modsArray.find((m) => m.title === title);

		if (targetMod) {
			currentModView.set(targetMod);
			skipHistoryUpdate = true;

			modView.scrollTo(0, 0);

			return true;
		}

		return false;
	}

	$effect(() => {
		if (skipHistoryUpdate) {
			skipHistoryUpdate = false;
			return;
		}

		history = [{ isMod: true, modName: mod.title }];
	});

	async function processInternalModLinks() {
		if (!description) return;

		const links = description.querySelectorAll("a");

		// Process each link
		for (const link of links) {
			if (link.href.startsWith("http")) {
				let result: internalModLinkData;

				if (linkCache.has(link.href)) {
					result = linkCache.get(link.href)!;
				} else {
					// Use the TypeScript implementation instead of invoking Rust
					const { isMod, modName } = isInternalModLink(link.href);
					result = { isMod, modName };
					linkCache.set(link.href, result);
				}

				if (result.isMod) {
					link.classList.add("internal-mod-link");
					// looks general better without this.
					// link.setAttribute("title", "Open in Mod Manager");
					link.setAttribute("data-internal-mod", result.modName);

					// Add direct click handler to each internal link
					link.onclick = (e) => {
						e.preventDefault();
						e.stopPropagation();

						history.unshift(result);

						console.log("hist:", history);

						setModViewByTitle(result.modName);

						return false;
					};
				}
			}
		}
	}

	const isModInstalled = async (mod: Mod) => {
		if (!mod) return false;

		await getAllInstalledMods(); // Ensure we have fresh data
		const status = installedMods.some((m) => m.name === mod.title);

		// Update the store outside of the reactive context
		setTimeout(() => {
			installationStatus.update((s) => ({
				...s,
				[mod.title]: status,
			}));
		}, 0);

		return status;
	};

	// This effect handles the description rendering
	$effect(() => {
		if (mod?.description) {
			Promise.resolve(marked(mod.description)).then((result) => {
				renderedDescription = result;
			});
		} else {
			renderedDescription = "";
		}
	});

	// Watch for changes to renderedDescription separately
	$effect(() => {
		if (renderedDescription) {
			// Use setTimeout to move to next microtask
			setTimeout(() => {
				processInternalModLinks();
			}, 0);
		}
	});

	function handleBack() {
		if (history.length <= 1) {
			currentModView.set(null);
			return;
		}

		history.shift();

		const modName = history[0].modName;
		setModViewByTitle(modName);
	}

	function handleClose() {
		currentModView.set(null);
	}

	onMount(async () => {
		window.addEventListener("auxclick", handleAuxClick);

		// Initial load of installed mods
		await getAllInstalledMods();

		// Check if the current mod is installed
		if (mod && !hasCheckedInstallation) {
			hasCheckedInstallation = true;
			setTimeout(() => {
				isModInstalled(mod);
			}, 0);
		}
	});

	// Handle mod changes from currentModView
	$effect(() => {
		const currentMod = untrack(() => $currentModView);

		if (currentMod) {
			// Check if this is a new mod
			if (
				!hasCheckedInstallation ||
				(mod && mod.title !== currentMod.title)
			) {
				hasCheckedInstallation = true;

				// Move installation check outside reactive context
				setTimeout(() => {
					isModInstalled(currentMod);
				}, 0);
			}
		}
	});

	// Handle loading of version data for special mods
	$effect(() => {
		const currentModTitle = mod?.title?.toLowerCase();
		if (
			currentModTitle === "steamodded" &&
			currentModTitle !== prevModTitle &&
			!versionLoadStarted
		) {
			prevModTitle = currentModTitle;
			versionLoadStarted = true;

			// Move version loading outside reactive context
			setTimeout(() => {
				loadSteamoddedVersions().then(() => {
					versionLoadStarted = false;
				});
			}, 0);
		} else if (
			currentModTitle === "talisman" &&
			currentModTitle !== prevModTitle &&
			!versionLoadStarted
		) {
			prevModTitle = currentModTitle;
			versionLoadStarted = true;

			// Move version loading outside reactive context
			setTimeout(() => {
				loadTalismanVersions().then(() => {
					versionLoadStarted = false;
				});
			}, 0);
		}
	});

	onDestroy(async () => {
		window.removeEventListener("auxclick", handleAuxClick);
		cachedVersions.set({ steamodded: [], talisman: [] });

		// Ensure installation status is updated before component unmounts
		if ($currentCategory === "Installed Mods") {
			await getAllInstalledMods();
			for (const mod of modsArray) {
				const isInstalled = installedMods.some(
					(m) => m.name === mod.title,
				);
				installationStatus.update((s) => ({
					...s,
					[mod.title]: isInstalled,
				}));
			}
		}
	});
</script>

<svelte:window
	on:keydown={(e) => {
		if (e.key === "Backspace" || e.key === "Escape") {
			handleBack();
		}
	}}
/>

<div
	class="mod-view default-scrollbar"
	transition:fade={{ duration: 300, easing: cubicOut }}
	bind:this={modView}
>
	<div class="mod-content">
		<div class="header-container">
			<div class="header">
				<button class="back-button" onclick={handleBack}>
					<ArrowLeft size={20} /> <span>Back</span>
				</button>

				{#if history.length > 1}
					<button
						transition:fade={{ duration: 300, easing: cubicOut }}
						onclick={handleClose}
						class="close-button"
					>
						<X size={20} />
					</button>
				{/if}
			</div>
		</div>

		<h2>{mod.title}</h2>
		<div class="content-grid">
			<div class="left-column">
				<div class="image-container">
					{#if !isDefaultCover(mod.image)}
						<button
							class="image-button"
							aria-label={`View full size image of ${mod.title}`}
						>
							<img
								src={mod.image}
								alt={mod.title}
								draggable="false"
							/>
						</button>
					{:else}
						<img
							src={mod.image}
							alt={mod.title}
							draggable="false"
						/>
					{/if}
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
				{#if mod.title.toLowerCase() === "talisman" && !$installationStatus[mod.title]}
					<div class="version-selector">
						{#if loadingVersions}
							<div class="loading-text">Loading versions...</div>
						{:else if talismanVersions.length === 0}
							<div class="loading-text">
								No versions available
							</div>
						{:else}
							<select
								bind:value={selectedVersion}
								disabled={$loadingStates[mod.title]}
							>
								<option value="newest" selected
									>latest (could be unstable)</option
								>
								{#each talismanVersions as version}
									<option value={version}>{version}</option>
								{/each}
							</select>
						{/if}
					</div>
				{/if}
				{#if mod.title.toLowerCase() === "steamodded" && !$installationStatus[mod.title]}
					<div class="version-selector">
						{#if loadingVersions}
							<div class="loading-text">Loading versions...</div>
						{:else if steamoddedVersions.length === 0}
							<div class="loading-text">
								No versions available
							</div>
						{:else}
							<select
								bind:value={selectedVersion}
								disabled={$loadingStates[mod.title]}
							>
								<option value="newest" selected
									>latest (could be unstable)</option
								>
								{#each steamoddedVersions as version}
									<option value={version}>{version}</option>
								{/each}
							</select>
						{/if}
					</div>
				{/if}
				<div class="mod-stats">
					<!-- <span><Clock size={16} /> {mod.lastUpdated}</span> -->
					<span><User size={16} /> {mod.publisher}</span>
				</div>
				{#if mod.repo}
					<button onclick={() => open(mod.repo)} class="repo-button">
						<Github size={16} /> Repository
					</button>
				{/if}
			</div>
			<div class="right-column">
				<div
					class="description"
					role="button"
					bind:this={description}
					tabindex="0"
					onclick={handleMarkdownClick}
					onkeydown={(e) => {
						if (e.key === "Enter" || e.key === " ") {
							handleMarkdownClick(e);
						}
					}}
				>
					{@html renderedDescription}
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	:global(.description > p > img) {
		width: 100%;
	}

	.mod-view {
		position: fixed;
		top: 0;
		right: 0;
		width: 100%;
		height: 100%;
		/* background: linear-gradient(to bottom, #393646, #4a4458); */
		background: rgba(133, 35, 27, 0.8);
		backdrop-filter: blur(20px);
		z-index: 1000;
		overflow-y: auto;
		font-family: "M6X11", sans-serif;
	}

	.mod-content {
		position: relative;

		/* max-width: 1000px; */
		padding: 3rem;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
	}

	.image-button {
		padding: 0;
		margin: 0;
		border: none;
		background: none;
		cursor: pointer;
		width: 100%;
		height: 100%;
		display: block;
		line-height: 0; /* Add this to remove any spacing */
		font-size: 0; /* Add this to remove any spacing */
	}

	h2 {
		margin-bottom: 2rem;
		font-size: 1.8rem;
	}

	.content-grid {
		display: grid;
		grid-template-columns: 350px 1fr;
		gap: 3rem;
	}

	.image-container {
		border-radius: 8px;
		height: 250px;
		overflow: hidden;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
	}

	img {
		width: 100%;
		height: 250px;
		object-fit: cover;
		transition: transform 0.2s ease;
		display: block;
	}

	img:hover {
		transform: scale(1.02);
	}

	.button-container {
		display: flex;
		gap: 0.5rem;
		margin: 1rem 0;
	}

	.download-button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		padding: 1rem;
		background: #56a786;
		color: #f4eee0;
		border: none;
		border-radius: 6px;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		font-family: "M6X11", sans-serif;
	}

	.download-button:hover:not(.installed) {
		background: #63b897;
		transform: translateY(-2px);
	}

	.download-button.installed {
		background: #808080;
		cursor: not-allowed;
	}

	.download-button:active:not(.installed) {
		transform: translateY(1px);
	}

	.delete-button {
		padding: 0.75rem;
		background: #c14139;
		color: #f4eee0;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.delete-button:hover {
		background: #d4524a;
		transform: translateY(-2px);
	}

	.mod-stats {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		font-size: 1.1rem;
		padding: 1rem;
		background: rgba(244, 238, 224, 0.1);
		border-radius: 6px;
		justify-content: center;
		align-items: center;
	}

	.mod-stats span {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: #f4eee0;
	}

	:global(.description > p > img) {
		width: 100%;
	}

	.description {
		font-size: 1.2rem;
		line-height: 1;
		color: #f4eee0;
		background: rgba(244, 238, 224, 0.05);
		padding: 1.25rem;
		border-radius: 6px;
		width: 50rem;
		line-height: 1.5;
	}

	/* Improved inline code styling */
	.description :global(code) {
		background: rgba(50, 50, 50, 0.7);
		color: #e6e1cf;
		padding: 0.2em 0.4em;
		border-radius: 3px;
		font-family: "Consolas", "Monaco", "Menlo", monospace;
		font-size: 0.75em;
	}

	/* Improved code block styling */
	.description :global(pre) {
		background: rgba(40, 40, 40, 0.8);
		padding: 1em;
		border-radius: 6px;
		overflow-x: auto;
		margin: 1em 0;
		border: 1px solid rgba(100, 100, 100, 0.3);
	}

	/* Style code within pre blocks differently than inline code */
	.description :global(pre code) {
		background: transparent;
		padding: 0;
		color: #f4eee0;
		display: block;
		line-height: 1.5;
		white-space: pre;
	}

	/* Add syntax highlighting colors */
	.description :global(.token.keyword),
	.description :global(.token.operator) {
		color: #ff7b72;
	}

	.description :global(.token.string),
	.description :global(.token.char) {
		color: #a5d6ff;
	}

	.description :global(.token.function),
	.description :global(.token.method) {
		color: #d2a8ff;
	}

	.description :global(.token.number) {
		color: #f8c555;
	}

	.description :global(.token.comment) {
		color: #8b949e;
		font-style: italic;
	}

	.description :global(.token.boolean),
	.description :global(.token.constant) {
		color: #79c0ff;
	}

	.header-container {
		position: absolute;
		top: 0;
		left: 0;
		height: 100%;
		width: 100%;

		z-index: 999;

		pointer-events: none;
	}

	.back-button {
		position: relative;
		/* top: 1rem;
		left: 1rem; */
		display: flex;
		align-items: center;
		gap: 0.5rem;
		background: rgba(244, 238, 224, 0.1);
		border: none;
		color: #f4eee0;
		padding: 0.5rem 1rem;
		border-radius: 6px;
		cursor: pointer;
		transition: all 0.2s ease;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		z-index: 100;

		pointer-events: auto;

		backdrop-filter: blur(20px) brightness(0.7);
	}

	.close-button {
		position: relative;
		/* top: 1rem;
		right: 1rem; */

		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.5rem;
		border-radius: 6px;

		background: rgba(244, 238, 224, 0.1);
		border: none;
		color: #f4eee0;

		cursor: pointer;
		transition: all 0.2s ease;

		font-family: "M6X11", sans-serif;
		font-size: 1rem;

		z-index: 100;
		pointer-events: auto;

		backdrop-filter: blur(20px) brightness(0.7);
	}

	.close-button:hover {
		scale: 1.1;
		background: rgba(244, 238, 224, 0.2);
	}

	.close-button:active {
		scale: 0.95;
		background: rgba(218, 212, 201, 0.1);
	}

	.back-button:hover {
		background: rgba(244, 238, 224, 0.2);
		transform: translateX(-5px);
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;

		box-sizing: border-box;
		position: sticky;

		top: 1rem;
		width: 100%;
		height: 2.5rem;

		padding: 0 1rem;
	}

	.description :global(h1),
	.description :global(h2),
	.description :global(h3),
	.description :global(h4) {
		margin-bottom: 0.5em;
		color: #f4eee0;
	}

	.description :global(p) {
		margin-bottom: 1em;
	}

	.description :global(ul),
	.description :global(ol) {
		margin-left: 1.5em;
		margin-bottom: 1em;
	}

	.description :global(li) {
		margin-bottom: 0.5em;
	}

	.description :global(a) {
		color: #56a786;
		text-decoration: none;
	}
	.description :global(a.internal-mod-link) {
		/* Use Balatro's gold color for internal mod links */
		color: #fdcf51 !important;
		position: relative;
	}

	.description :global(a.internal-mod-link::after) {
		display: inline-block;
		margin-left: 3px;
		transform: rotate(-45deg);
		font-weight: bold;
	}

	.description :global(a.internal-mod-link:hover) {
		text-decoration: underline;
		filter: brightness(1.2);
	}

	.description :global(a.internal-mod-link:hover::before) {
		content: "Open in Mod Manager";
		position: absolute;
		bottom: -35px;
		left: 0;
		background: rgba(0, 0, 0, 0.8);
		color: white;
		padding: 4px 8px;
		border-radius: 4px;
		font-size: 0.8em;
		white-space: nowrap;
		z-index: 10;
	}

	.description :global(a:hover) {
		text-decoration: underline;
		z-index: 10;
	}

	.description :global(blockquote) {
		border-left: 3px solid #56a786;
		margin: 1em 0;
		padding-left: 1em;
		color: rgba(244, 238, 224, 0.8);
	}

	.description :global(a) {
		-webkit-user-drag: none;
		user-select: none;
		-moz-user-select: none;
		-webkit-user-select: none;
		-ms-user-select: none;
	}

	.delete-button:active {
		transform: translateY(1px);
	}
	/* Make sure the original image is clickable */
	.image-container img {
		cursor: default;
	}

	/* .image-container .clickable { */
	/* 	cursor: pointer; */
	/* } */

	@media (max-width: 1360px) {
		.content-grid {
			grid-template-columns: 1fr;
		}
		.image-container {
			width: 100%;
			height: 350px;

			& > img {
				height: 100%;
			}
		}

		.image-button {
			height: 100%;

			& > img {
				height: 100%;
			}
		}

		.right-column {
			bottom: 2rem;
			position: relative;
		}

		.mod-content {
			width: 100%;
			max-width: 100%;
			box-sizing: border-box;
		}

		.right-column {
			display: flex;
			flex-direction: column;
			align-items: center;
		}
	}

	.download-button:disabled {
		opacity: 0.8;
		cursor: not-allowed;
	}

	.version-selector {
		margin-bottom: 1rem;
		width: 100%;
	}

	.loading-text {
		/* width: 100%; */
		padding: 0.75rem;
		background: rgba(133, 35, 27, 0.8);
		color: #f4eee0;
		border: 1px solid rgba(193, 65, 57, 0.6);
		border-radius: 6px;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		text-align: center;
	}

	.version-selector select {
		width: 100%;
		padding: 0.75rem;
		background: rgba(133, 35, 27, 0.8);
		color: #f4eee0;
		border: 1px solid rgba(193, 65, 57, 0.6);
		border-radius: 6px;
		font-family: "M6X11", sans-serif;
		cursor: pointer;
		font-size: 1rem;
		transition: all 0.2s ease;
		-webkit-appearance: none;
		-moz-appearance: none;
		appearance: none;
		background-image: url("data:image/svg+xml;charset=US-ASCII,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%22292.4%22%20height%3D%22292.4%22%3E%3Cpath%20fill%3D%22%23F4EEE0%22%20d%3D%22M287%2069.4a17.6%2017.6%200%200%200-13-5.4H18.4c-5%200-9.3%201.8-12.9%205.4A17.6%2017.6%200%200%200%200%2082.2c0%205%201.8%209.3%205.4%2012.9l128%20127.9c3.6%203.6%207.8%205.4%2012.8%205.4s9.2-1.8%2012.8-5.4L287%2095c3.5-3.5%205.4-7.8%205.4-12.8%200-5-1.9-9.2-5.4-12.8z%22%2F%3E%3C%2Fsvg%3E");
		background-repeat: no-repeat;
		background-position: right 0.7em top 50%;
		background-size: 0.65em auto;
		padding-right: 2.5em;
	}

	.version-selector select:hover:not(:disabled) {
		background-color: rgba(133, 35, 27, 0.9);
		border-color: rgba(193, 65, 57, 0.8);
		transform: translateY(-2px);
	}

	.version-selector select:disabled {
		opacity: 0.7;
		cursor: not-allowed;
	}

	.repo-button {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		width: 100%;
		height: 3rem;
		padding: 0.75rem 1.5rem;
		background: #2b3137;
		color: #f4eee0;
		border: none;
		outline: #1b2127 solid 2px;
		border-radius: 4px;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		text-decoration: none;
		margin-top: 1rem;
		justify-content: center;
	}

	.repo-button:hover {
		background: #3b4147;
		transform: translateY(-2px);
	}
	.version-selector select option {
		background: rgba(133, 35, 27, 0.9);
		color: #f4eee0;
		padding: 0.75rem;
	}

	.description :global(a) {
		color: #56a786;
		text-decoration: none;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.description :global(a:hover) {
		text-decoration: underline;
		filter: brightness(1.2);
	}
</style>
