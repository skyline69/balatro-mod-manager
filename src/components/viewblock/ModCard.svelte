<script lang="ts">
	import type { Mod } from "../../stores/modStore";
	import { Download, Trash2, RefreshCw } from "lucide-svelte";
import {
	installationStatus,
	loadingStates2 as loadingStates,
	modEnabledStore,
	updateAvailableStore,
} from "../../stores/modStore";
import { descriptionsStore } from "../../stores/descriptions";
	import { stripMarkdown } from "../../utils/helpers";
	import { invoke } from "@tauri-apps/api/core";
	import { lovelyPopupStore } from "../../stores/modStore";
	import { forceRefreshCache } from "../../stores/modCache";
import LazyImage from "../common/LazyImage.svelte";
	import { cardScale } from "../../stores/ui";
import { onMount } from "svelte";
import { isLinuxPlatform } from "$lib/platform";

	interface Props {
		mod: Mod;
		onmodclick?: (mod: Mod) => void;
		oninstallclick?: (mod: Mod) => void;
		onuninstallclick?: (mod: Mod) => void;
		onToggleEnabled?: () => Promise<void>;
		deferImages?: boolean;
	}

	let {
		mod,
		oninstallclick,
		onuninstallclick,
		onmodclick,
		onToggleEnabled,
		deferImages = false,
	}: Props = $props();

	let isEnabled = $state(true); // Default to enabled if not yet checked
	let enabledChecked = false;
	let isLinux = false;
	let descriptionText = $derived(
		$descriptionsStore[mod.title] ?? mod.description ?? "",
	);

	// Load the enabled state whenever the mod changes or when installationStatus changes
	$effect(() => {
		if (
			$installationStatus[mod.title] &&
			!enabledChecked &&
			$modEnabledStore[mod.title] === undefined
		) {
			enabledChecked = true;
			checkModEnabled(mod.title);
		}
	});

	onMount(async () => {
		isLinux = await isLinuxPlatform();
	});

	async function checkModEnabled(modName: string) {
		try {
			const enabled = await invoke<boolean>("is_mod_enabled", {
				modName,
			});

			modEnabledStore.update((enabledMods: Record<string, boolean>) => ({
				...enabledMods,
				[modName]: enabled,
			}));

			// Also update local variable for reactive binding
			isEnabled = enabled;
		} catch (error) {
			console.error(
				`Failed to check if mod ${modName} is enabled:`,
				error,
			);
			// Default to enabled on error
			modEnabledStore.update((enabledMods: Record<string, boolean>) => ({
				...enabledMods,
				[modName]: true,
			}));
			isEnabled = true;
		}
	}

	async function toggleModEnabled(e: Event) {
		e.stopPropagation();
		try {
			const currentState = $modEnabledStore[mod.title] ?? isEnabled;
			const newState = !currentState;

			await invoke("toggle_mod_enabled", {
				modName: mod.title,
				enabled: newState,
			});

			// Update both the store and local variable
			modEnabledStore.update((enabledMods) => ({
				...enabledMods,
				[mod.title]: newState,
			}));
			isEnabled = newState;

			// Call the parent callback to update the filtered lists
			if (onToggleEnabled) {
				await onToggleEnabled();
			}
		} catch (error) {
			console.error(
				`Failed to toggle mod ${mod.title} enabled state:`,
				error,
			);
		}
	}
	function installMod(e: Event) {
		e.stopPropagation();
		// Guard: don't allow re-entrancy or duplicate installs
		if ($loadingStates[mod.title] || $installationStatus[mod.title]) return;
		if (mod.title.toLowerCase() === "steamodded") {
			// Set loading immediately to prevent multiple clicks while fetching URL
			loadingStates.update((s) => ({ ...s, [mod.title]: true }));
			fetchAndInstallLatestSteamodded().catch(() => {
				// ensure loading is cleared on early failure
				loadingStates.update((s) => ({ ...s, [mod.title]: false }));
			});
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
			throw error;
		}
	}

	async function installModFromURL(url: string, folder_name: string = "") {
		const wasInstalled = Boolean($installationStatus[mod.title]);
		let desiredEnabledState = true;

		if (wasInstalled) {
			let previousEnabled = $modEnabledStore[mod.title];
			if (previousEnabled === undefined) {
				try {
					previousEnabled = await invoke<boolean>("is_mod_enabled", {
						modName: mod.title,
					});
				} catch (error) {
					console.error(
						`Failed to read existing enabled state for ${mod.title}:`,
						error,
					);
				}
			}

			if (previousEnabled !== undefined) {
				desiredEnabledState = previousEnabled;
			} else {
				desiredEnabledState = isEnabled;
			}
		}

		try {
			loadingStates.update((s) => ({ ...s, [mod.title]: true }));

			// Show a warning if Lovely injector is missing (do not block installation)
			if (!isLinux) {
				try {
					const present = await invoke<boolean>("is_lovely_installed");
					if (!present) {
						lovelyPopupStore.set({ visible: true });
					}
				} catch (_) {
					/* ignore */
				}
			}

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
			updateAvailableStore.update((updates) => ({
				...updates,
				[mod.title]: false,
			}));

			modEnabledStore.update((enabledMods) => ({
				...enabledMods,
				[mod.title]: desiredEnabledState,
			}));
			isEnabled = desiredEnabledState;

			// Manually check mod enabled status after installation
			setTimeout(() => checkModEnabled(mod.title), 500);

			// After install, verify Lovely is still present
			if (!isLinux) {
				try {
					const present = await invoke<boolean>("is_lovely_installed");
					if (!present) {
						lovelyPopupStore.set({ visible: true });
					}
				} catch (_) {
					/* ignore */
				}
			}
		} catch (error) {
			console.error("Failed to install mod:", error);
		} finally {
			loadingStates.update((s) => ({ ...s, [mod.title]: false }));
			// Keep cache in sync so other views reflect installation immediately
			try {
				await forceRefreshCache();
			} catch (_) {
				/* ignore */
			}
		}
	}
	// Truncate description based on current card scale; this avoids overflow
	// even when CSS multi-line clamp support is inconsistent across platforms.
    function truncateDynamic(text: string, scale: number): string {
        if (!text) return "";
        const lines = scale <= 0.85 ? 1 : 2;
        // width and font-size scale together → per-line capacity ~ constant
        const basePerLine = 38;
        const maxChars = Math.max(18, basePerLine * lines);
        return text.length > maxChars
            ? text.slice(0, maxChars).trimEnd() + "..."
            : text;
    }
</script>

<div
	class="mod-card"
	class:compact={$cardScale <= 0.85}
	onclick={openModView}
	onkeydown={(e) => e.key === "Enter" && openModView()}
	role="button"
	tabindex="0"
	style="--orig-color1: {mod.colors.color1}; --orig-color2: {mod.colors
		.color2};"
>
	<div class="mod-image">
		<LazyImage
			src={mod.image}
			fallbackSrc={mod.imageFallback || "/images/cover.jpg"}
			alt={mod.title}
			cacheTitle={mod.title}
			deferLoad={deferImages}
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
		{#if descriptionText && descriptionText.trim().length > 0}
			<p>{truncateDynamic(stripMarkdown(descriptionText), $cardScale)}</p>
		{:else}
			<div class="desc-skeleton" aria-hidden="true">
				<div class="line" style="width: 92%"></div>
				<div class="line" style="width: 84%"></div>
				<div class="line" style="width: 68%"></div>
			</div>
		{/if}
	</div>

	<div class="button-container">
		{#if $installationStatus[mod.title]}
			<!-- Enable/Disable button (only shown when mod is installed) -->
			<button
				class="toggle-button"
				class:enabled={$modEnabledStore[mod.title] ?? isEnabled}
				class:disabled={!($modEnabledStore[mod.title] ?? isEnabled)}
				title={($modEnabledStore[mod.title] ?? isEnabled)
					? "Disable Mod"
					: "Enable Mod"}
				onclick={toggleModEnabled}
			>
				{#if $modEnabledStore[mod.title] ?? isEnabled}
					ON
				{:else}
					OFF
				{/if}
			</button>
		{/if}

		{#if $installationStatus[mod.title] && $updateAvailableStore[mod.title]}
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
	width: calc(300px * var(--card-scale, 1));
	max-width: calc(300px * var(--card-scale, 1));
	height: calc(330px * var(--card-scale, 1));
	content-visibility: auto;
	contain-intrinsic-size: 300px 330px;
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
		will-change: transform;
		backface-visibility: hidden;
	}

	.mod-card:hover {
		animation: stripe-slide-up 1.5s linear infinite;
		scale: 1.05;
	}

	:global([data-platform="linux"]) .mod-card:hover {
		animation: none;
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
		height: calc(150px * var(--card-scale, 1));
	}

	/* Image styling handled inside LazyImage */

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
		word-break: break-word;
		overflow-wrap: anywhere;
		text-overflow: ellipsis;
	}

	.mod-info h3 {
		color: #fdcf51;
		font-size: calc(1.5rem * var(--card-scale, 1));
		margin-bottom: 0.2rem;
	}

	.mod-info p {
		color: #f4eee0;
		font-size: calc(1rem * var(--card-scale, 1));
		line-height: 1.2;
	}

	/* Tighten description to 1 line for compact cards */
	.mod-card.compact .mod-info > p {
		-webkit-line-clamp: 1;
		line-clamp: 1;
	}

	/* Description skeleton */
	.desc-skeleton {
		margin-top: 0.2rem;
	}
	.desc-skeleton .line {
		height: 12px;
		margin: 6px 0;
		border-radius: 6px;
		background: linear-gradient(
			90deg,
			rgba(255, 255, 255, 0.08) 25%,
			rgba(255, 255, 255, 0.18) 37%,
			rgba(255, 255, 255, 0.08) 63%
		);
		background-size: 400% 100%;
		animation: shimmer 1.2s ease-in-out infinite;
	}

	@keyframes shimmer {
		0% {
			background-position: 100% 0;
		}
		100% {
			background-position: 0 0;
		}
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
		padding: calc(0.75rem * var(--card-scale, 1));
		border: none;
		border-radius: 4px;
		font-family: "M6X11", sans-serif;
		font-size: calc(1rem * var(--card-scale, 1));
		cursor: pointer;
		transition: all 0.2s ease;
		/* Add these properties to prevent resizing */
		min-height: calc(42px * var(--card-scale, 1)); /* Set explicit height */
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

	/* Compact adjustments: make primary buttons a touch smaller */
	.mod-card.compact .download-button,
	.mod-card.compact .update-button {
		padding: calc(0.55rem * var(--card-scale, 1));
		min-height: calc(36px * var(--card-scale, 1));
		font-size: calc(0.9rem * var(--card-scale, 1));
	}

	.mod-card.compact .toggle-button {
		transform: translateY(1px);
	}

	.mod-card.compact .button-container {
		bottom: 0.7rem;
	}

	.delete-button {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: calc(42px * var(--card-scale, 1));
		height: calc(42px * var(--card-scale, 1));
		padding: calc(8px * var(--card-scale, 1));
		background: #c14139;
		color: #f4eee0;
		border: none;
		outline: #a13029 solid 2px;
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.2s ease;
		font-family: "M6X11", sans-serif;
		font-size: calc(1rem * var(--card-scale, 1));
	}

	.delete-button:hover {
		background: #d4524a;
		transform: translateY(-2px);
	}

	.delete-button:active {
		transform: translateY(1px);
	}

	/* Enable/Disable toggle button styles */
	.toggle-button {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: calc(42px * var(--card-scale, 1));
		height: calc(42px * var(--card-scale, 1));
		padding: calc(8px * var(--card-scale, 1));
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.2s ease;
		color: white;
		border: none;
		flex-shrink: 0;
		font-family: "M6X11", sans-serif;
		font-size: calc(1.1rem * var(--card-scale, 1));
	}

	.toggle-button.enabled {
		background: #27ae60; /* Bright green when enabled */
		outline: #219653 solid 2px;
	}

	.toggle-button.disabled {
		background: #7f8c8d; /* Gray when disabled, instead of red */
		outline: #636e72 solid 2px;
	}

	.toggle-button:hover.enabled {
		background: #2ecc71; /* Lighter green on hover */
		transform: translateY(-2px);
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
	}

	.toggle-button:hover.disabled {
		background: #95a5a6; /* Lighter gray on hover */
		transform: translateY(-2px);
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
	}

	.toggle-button:active {
		transform: translateY(1px);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
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
		width: calc(16px * var(--card-scale, 1));
		height: calc(16px * var(--card-scale, 1));
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
