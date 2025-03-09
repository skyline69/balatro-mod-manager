<script lang="ts">
	import type { Mod } from "../../stores/modStore";
	import { Download, Trash2, RefreshCw } from "lucide-svelte";
	import {
		installationStatus,
		loadingStates2 as loadingStates,
	} from "../../stores/modStore";
	import { stripMarkdown, truncateText } from "../../utils/helpers";
	import { invoke } from "@tauri-apps/api/core";
	import { writable } from "svelte/store";

	// Store to track which mods have updates available
	const updateAvailable = writable<Record<string, boolean>>({});

	interface Props {
		mod: Mod;
		onmodclick?: (mod: Mod) => void;
		oninstallclick?: (mod: Mod) => void;
		onuninstallclick?: (mod: Mod) => void;
	}

	let { mod, oninstallclick, onuninstallclick, onmodclick }: Props = $props();

	// Check if an update is available when component mounts
	$effect(() => {
		checkForUpdate(mod.title);
	});

	async function checkForUpdate(modName: string) {
		try {
			const hasUpdate = await invoke<boolean>("mod_update_available", {
				modName,
			});

			updateAvailable.update((updates) => ({
				...updates,
				[modName]: hasUpdate,
			}));
		} catch (error) {
			console.error("Failed to check for updates:", error);
		}
	}

	function installMod(e: Event) {
		e.stopPropagation();
		if (mod.title.toLowerCase() === "steamodded") {
			fetchAndInstallLatestSteamodded();
		} else if (oninstallclick) {
			oninstallclick(mod);
		}
	}

	function updateMod(e: Event) {
		e.stopPropagation();
		// Reuse the install logic but for updating
		if (mod.title.toLowerCase() === "steamodded") {
			fetchAndInstallLatestSteamodded();
		} else if (oninstallclick) {
			oninstallclick(mod);
		}
	}

	function uninstallMod(e: Event) {
		e.stopPropagation();
		if (onuninstallclick) onuninstallclick(mod);
	}

	function openModView() {
		if (onmodclick) onmodclick(mod);
	}

	async function fetchAndInstallLatestSteamodded() {
		try {
			const latestReleaseURL = await invoke<string>(
				"get_latest_steamodded_release",
			);
			await installModFromURL(latestReleaseURL);
		} catch (error) {
			console.error("Failed to get latest Steamodded release:", error);
		}
	}

	async function installModFromURL(url: string, folder_name: string = "") {
		try {
			loadingStates.update((s) => ({ ...s, [mod.title]: true }));

			if (!url.startsWith("http")) {
				console.error("Invalid URL format:", url);
				throw new Error(`Invalid URL format: ${url}`);
			}

			// Use mod title as fallback if folder_name is empty
			const folderName = folder_name || mod.title || "";

			const installedPath = await invoke<string>("install_mod", {
				url,
				folderName,
			});

			await invoke("add_installed_mod", {
				name: mod.title,
				path: installedPath,
				dependencies: mod.requires_steamodded ? ["Steamodded"] : [],
				currentVersion: mod.version || "",
			});

			installationStatus.update((s) => ({ ...s, [mod.title]: true }));

			// After installing/updating, reset update status
			updateAvailable.update((updates) => ({
				...updates,
				[mod.title]: false,
			}));
		} catch (error) {
			console.error("Failed to install mod:", error);
		} finally {
			loadingStates.update((s) => ({ ...s, [mod.title]: false }));
		}
	}
</script>

<div
	class="mod-card"
	onclick={openModView}
	onkeydown={(e) => e.key === "Enter" && openModView()}
	role="button"
	tabindex="0"
	style="--orig-color1: {mod.colors.color1}; --orig-color2: {mod.colors
		.color2};"
>
	<div class="mod-image">
		<img src={mod.image} alt={mod.title} draggable="false" />

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
		{#if $installationStatus[mod.title] && $updateAvailable[mod.title]}
			<!-- Update button (when installed and update available) -->
			<button
				class="update-button"
				onclick={updateMod}
				disabled={$loadingStates[mod.title]}
			>
				{#if $loadingStates[mod.title]}
					<div class="spinner"></div>
				{:else}
					<RefreshCw size={18} />
					Update Mod
				{/if}
			</button>
		{:else}
			<!-- Regular download/installed button -->
			<button
				class="download-button"
				class:installed={$installationStatus[mod.title]}
				disabled={$installationStatus[mod.title] ||
					$loadingStates[mod.title]}
				onclick={installMod}
			>
				{#if $loadingStates[mod.title]}
					<div class="spinner"></div>
				{:else}
					<Download size={18} />
					{$installationStatus[mod.title] ? "Installed" : "Download"}
				{/if}
			</button>
		{/if}

		{#if $installationStatus[mod.title]}
			<button
				class="delete-button"
				title="Remove Mod"
				onclick={uninstallMod}
			>
				<Trash2 size={18} />
			</button>
		{/if}
	</div>
</div>

<style>
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
		max-width: 300px;
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
		animation: stripe-slide-up 1.5s linear infinite;
		scale: 1.05;
	}

	@keyframes stripe-slide-up {
		0% {
			background-position: 0 0;
		}
		100% {
			background-position: 0 -55px;
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

	.mod-info > p {
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		display: -webkit-box;
		-webkit-box-orient: vertical;
		padding: 0 0.1rem;
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

	.download-button,
	.update-button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		padding: 0.75rem;
		border: none;
		border-radius: 4px;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		/* Add these properties to prevent resizing */
		min-height: 42px; /* Set explicit height */
		position: relative; /* For absolute positioning of spinner */
	}

	.download-button {
		background: #56a786;
		color: #f4eee0;
		outline: #459373 solid 2px;
	}

	.update-button {
		background: #3498db;
		color: #f4eee0;
		outline: #2980b9 solid 2px;
	}

	.update-button:hover {
		background: #5dade2; /* Lighter blue on hover */
		transform: translateY(-2px);
	}

	.update-button:active {
		transform: translateY(1px);
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

	.delete-button:hover {
		background: #d4524a;
		transform: translateY(-2px);
	}

	.delete-button:active {
		transform: translateY(1px);
	}

	.download-button:disabled,
	.update-button:disabled {
		opacity: 0.8;
		cursor: not-allowed;
	}

	@media (max-width: 1160px) {
		.mod-card {
			width: 100%;
		}
	}

	.spinner {
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-top: 2px solid #ffffff;
		border-radius: 50%;
		width: 16px;
		height: 16px;
		animation: spin 1s linear infinite;
		/* Center the spinner while maintaining button size */
		margin: 0 auto;
		display: inline-block;
	}

	@keyframes spin {
		0% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(360deg);
		}
	}
</style>
