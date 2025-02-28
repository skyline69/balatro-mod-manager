<script lang="ts">
	import { fade } from "svelte/transition";
	import { cubicOut } from "svelte/easing";
	import { Download, Trash2, User, ArrowLeft, Github } from "lucide-svelte";
	import { onMount, onDestroy } from "svelte";
	import { open } from "@tauri-apps/plugin-shell";
	import {
		currentModView,
		installationStatus,
		loadingStates2 as loadingStates,
		uninstallDialogStore,
	} from "../../stores/modStore";
	import type { InstalledMod, Mod } from "../../stores/modStore";
	import { marked } from "marked";
	import { invoke } from "@tauri-apps/api/core";
	import { cachedVersions } from "../../stores/modStore";
	const VERSION_CACHE_DURATION = 60 * 60 * 1000;
	const { mod, onCheckDependencies } = $props<{
		mod: Mod;
		onCheckDependencies?: (event: {
			steamodded: boolean;
			talisman: boolean;
		}) => void;
	}>();
	const isDefaultCover = (imageUrl: string) => imageUrl.includes("cover.jpg");
	function handleAuxClick(event: MouseEvent) {
		if (event.button === 3) {
			event.preventDefault();
			handleClose();
		}
	}
	// async function openImagePopup() {
	// 	console.log("Opening popup with image:", {
	// 		imageType: typeof mod.image,
	// 		imageLength: mod.image.length,
	// 		imagePreview: mod.image.substring(0, 100) + "...",
	// 	});
	//
	// 	if (!isDefaultCover(mod.image)) {
	// 		await invoke("open_image_popup", {
	// 			imageUrl: mod.image,
	// 			title: mod.title,
	// 		});
	// 	}
	// }

	let installedMods: InstalledMod[] = [];
	let steamoddedVersions = $state<string[]>([]);
	let talismanVersions = $state<string[]>([]);
	let selectedVersion = $state("newest");
	let loadingVersions = $state(false);
	let renderedDescription = $state("");

	let versionLoadStarted = false;
	let prevModTitle = "";

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
		if (anchor && anchor.href.startsWith("http")) {
			event.preventDefault();
			open(anchor.href).catch((e) =>
				console.error("Failed to open link:", e),
			);
		}
	}
	const isModInstalled = async (mod: Mod) => {
		await getAllInstalledMods();
		const status = installedMods.some((m) => m.name === mod.title);
		installationStatus.update((s) => ({ ...s, [mod.title]: status }));
		return status;
	};
	$effect(() => {
		if (mod?.description) {
			Promise.resolve(marked(mod.description)).then((result) => {
				renderedDescription = result;
			});
		} else {
			renderedDescription = "";
		}
	});
	function handleClose() {
		currentModView.set(null);
	}
	onMount(async () => {
		window.addEventListener("auxclick", handleAuxClick);
		if (mod) {
			await getAllInstalledMods();
			await isModInstalled(mod);
		}
	});
	$effect(() => {
		const currentModTitle = mod?.title?.toLowerCase();
		if (
			currentModTitle === "steamodded" &&
			currentModTitle !== prevModTitle &&
			!versionLoadStarted
		) {
			prevModTitle = currentModTitle;
			versionLoadStarted = true;
			loadSteamoddedVersions().then(() => {
				versionLoadStarted = false;
			});
		} else if (
			currentModTitle === "talisman" &&
			currentModTitle !== prevModTitle &&
			!versionLoadStarted
		) {
			prevModTitle = currentModTitle;
			versionLoadStarted = true;
			loadTalismanVersions().then(() => {
				versionLoadStarted = false;
			});
		}
	});
	onDestroy(async () => {
		window.removeEventListener("auxclick", handleAuxClick);
		cachedVersions.set({ steamodded: [], talisman: [] });
	});
</script>

<svelte:window
	on:keydown={(e) => {
		if (e.key === "Backspace" || e.key === "Escape") {
			handleClose();
		}
	}}
/>

<div
	class="mod-view default-scrollbar"
	transition:fade={{ duration: 300, easing: cubicOut }}
>
	<button class="back-button" onclick={handleClose}>
		<ArrowLeft size={20} /> <span>Back</span>
	</button>
	<div class="mod-content">
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
		max-width: 1000px;
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

	.back-button {
		position: absolute;
		top: 1rem;
		left: 1rem;
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
	}

	.back-button:hover {
		background: rgba(244, 238, 224, 0.2);
		transform: translateX(-5px);
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

	.description :global(a:hover) {
		text-decoration: underline;
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

	@media (max-width: 1160px) {
		.content-grid {
			grid-template-columns: 1fr;
		}
		.image-container {
			width: 100%;
		}
		.right-column {
			bottom: 2rem;
			position: relative;
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
