<script lang="ts">
	import PathSelector from "../PathSelector.svelte";
	import { Settings2, RefreshCw, Folder, Save } from "lucide-svelte";
	import { addMessage } from "$lib/stores";
	import { onMount } from "svelte";
	import { invoke } from "@tauri-apps/api/core";
	import { backgroundEnabled } from "../../stores/modStore";

	let isReindexing = false;
	let isClearingCache = false;
	let isConsoleEnabled = false;
	let isBackgroundAnimationEnabled = false;
	let lastReindexStats = {
		removedFiles: 0,
		cleanedEntries: 0,
	};
	let isDiscordRpcEnabled = false;

	let linuxPrefix = "";
	let operatingSystem = "";

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
			addMessage(
				"Failed to update Discord Rich Presence status",
				"error",
			);
		}
	}

	async function openModsFolder() {
		try {
			// Get the repository path (which should be config_dir/Balatro/mod_index)
			const modsFolderPath: string = await invoke("get_mods_folder");

			// Get the parent directory (config_dir/Balatro) by finding the last path separator
			const lastSeparatorIndex = Math.max(
				modsFolderPath.lastIndexOf("/"),
				modsFolderPath.lastIndexOf("\\"),
			);
			if (lastSeparatorIndex === -1) {
				addMessage(
					"Failed to determine the parent directory of the repository path.",
					"error",
				);
				return;
			}

			const parentPath = modsFolderPath.substring(0, lastSeparatorIndex);
			const separator = modsFolderPath.includes("/") ? "/" : "\\"; // Determine the separator used in the path

			// Construct the mods path
			const modsPath = `${parentPath}${separator}Mods`;

			// Check if the path exists
			const pathExists = await invoke("path_exists", { path: modsPath });

			if (!pathExists) {
				addMessage(
					"Mods directory not found. It might not have been created yet.",
					"warning",
				);
				addMessage(
					"Install a mod using the mod manager to create the mods directory.",
					"info",
				);
				return;
			}

			// Open the directory
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

	async function handleLinuxPrefixChange() {
		const newValue = linuxPrefix;
		try {
			await invoke("set_linux_prefix", { path: newValue });
			addMessage(
				`Linux prefix set to ${newValue}`,
				"success",
			);
		} catch (error) {
			console.error("Failed to set prefix:", error);
			addMessage("Failed to update prefix", "error");
		}
	}

	onMount(async () => {
		try {
			isDiscordRpcEnabled = await invoke("get_discord_rpc_status");
		} catch (error) {
			console.error("Failed to get Discord RPC status:", error);
			addMessage("Error fetching Discord Rich Presence status", "error");
		}
		try {
			isConsoleEnabled = await invoke("get_lovely_console_status");
		} catch (error) {
			console.error("Failed to get console status:", error);
			addMessage("Error fetching Lovely Console status", "error");
		}
		try {
			isBackgroundAnimationEnabled = await invoke("get_background_state");
			backgroundEnabled.set(isBackgroundAnimationEnabled);
		} catch (error) {
			console.error("Failed to get background status:", error);
			addMessage("Error fetching background animation status", "error");
		}
		try {
			linuxPrefix = await invoke("get_linux_prefix");
		} catch (error) {
			console.error("Failed to get Linux prefix:", error);
			addMessage("Error fetching Linux prefix", "error");
		}

		operatingSystem = await invoke("get_operating_system");
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
				on:click={clearCache}
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
					on:click={openModsFolder}
					title="Open mods folder"
				>
					<Folder size={20} />
					Open Mods Folder
				</button>

				<p class="description">
					Open the folder where mods are stored on your system.
				</p>

				<button
					class="reindex-button"
					on:click={performReindexMods}
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
						<span
							>Files removed: {lastReindexStats.removedFiles}</span
						>
						<span
							>Database entries cleaned: {lastReindexStats.cleanedEntries}</span
						>
					</div>
				{/if}
				<p class="description-small">
					Performs consistency check on the mod database. Will only
					remove:
					<br />• Database entries for missing mod installations
				</p>
								{#if operatingSystem == "linux"}

					<input
						type="text"
						bind:value={linuxPrefix}
						placeholder="Prefix"
						class="prefix-input"
					/>

					<button
						class="prefix-button"
						on:click={handleLinuxPrefixChange}
						title="Update Prefix"
					>
						<Save size={20} /> <!-- change to Save -->
						Save Prefix
					</button>

					<p class="description-small">Changes the prefix for Linux. Recommended is to use wine/proton.</p>
				{/if}
			</div>
			<h3>Appearance</h3>
			<div class="console-settings">
				<span class="label-text">Enable Background Animation</span>
				<div class="switch-container">
					<label class="switch">
						<input
							type="checkbox"
							checked={isBackgroundAnimationEnabled}
							on:change={handleBackgroundAnimationChange}
						/> <span class="slider"></span>
					</label>
				</div>
			</div>
			<p class="description-small">
				Enable or disable the animated background. Disabling may improve
				performance on low-end devices.
			</p>

			<div class="console-settings">
				<span class="label-text">Enable Discord Rich Presence</span>
				<div class="switch-container">
					<label class="switch">
						<input
							type="checkbox"
							checked={isDiscordRpcEnabled}
							on:change={handleDiscordRpcChange}
						/> <span class="slider"></span>
					</label>
				</div>
			</div>
			<p class="description-small">
				Show your Balatro activity in Discord. Displays your current
				status and mod manager usage.
			</p>

			<h3>Developer Options</h3>
			<div class="console-settings">
				<span class="label-text">Enable Lovely Console</span>
				<div class="switch-container">
					<label class="switch">
						<input
							type="checkbox"
							checked={isConsoleEnabled}
							on:change={handleConsoleChange}
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
		color: #fdcf51;
	}
	h3 {
		font-size: 1.8rem;
		margin-bottom: 1rem;
		align-self: flex-start;
		color: #fdcf51;
	}
	.content {
		flex: 1;
	}
	.reindex-button {
		background: #56a786;
		color: #f4eee0;
		border: none;
		outline: #459373 solid 2px;
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
		background: #74cca8;
		transform: translateY(-2px);
	}
	.throbber {
		width: 20px;
		height: 20px;
		border: 3px solid #f4eee0;
		border-radius: 50%;
		border-top-color: transparent;
		animation: spin 1s linear infinite;
	}
	.warning {
		color: #ffd700;
		font-size: 1.1rem;
		border-left: 3px solid #ffd700;
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
		.prefix-button {
		background: #ee8ed7;
		color: #f4eee0;
		border: none;
		outline: #ca70b5 solid 2px;
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
	.prefix-button:hover {
		background: #fba9e8;
		transform: translateY(-2px);
	}
	.prefix-input {
		width: auto;
		margin: 0.5rem 0;
	}
	input[type="text"] {
		padding: 0.75rem;
		border: 2px solid #fda200;
		border-radius: 8px;
		background-color: #c88000;
		font-family: "M6X11", sans-serif;
		color: white;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		-webkit-user-select: none;
		user-select: none;
	}
	input[type="text"]:hover:not(:disabled) {
		border-color: #f4eee0;
	}
	input[type="text"]::placeholder {
		color: white;
		-webkit-user-select: none;
		user-select: none;
	}
	.clear-cache-button {
		background: #6d28d9;
		color: #f4eee0;
		border: none;
		outline: #5b21b6 solid 2px;
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
		background: #7c3aed;
		transform: translateY(-2px);
	}
	.clear-cache-button:disabled {
		cursor: not-allowed;
		opacity: 0.8;
		transform: none;
	}

	.open-folder-button {
		background: #4caf50;
		color: #f4eee0;
		border: none;
		outline: #3d8b40 solid 2px;
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
		background: #45a049;
		transform: translateY(-2px);
	}

	.open-folder-button:active {
		transform: translateY(1px);
	}
	.description {
		color: #f4eee0;
		font-size: 1.2rem;
		margin-top: 0.5rem;
		opacity: 0.9;
		max-width: 400px;
		line-height: 1.4;
	} /* Custom Toggle Switch Styles */
	.description-small {
		/* color a bit grayer but still light */
		color: #c4c2c2;
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
		color: #f4eee0;
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
		background-color: #f87171;
		border: 2px solid #fc4747;
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
		background-color: #f4eee0;
		/* do a gray outline */
		outline: 2px solid #9e9a90;
		transition: 0.3s;
		border-radius: 5px;
	} /* Enabled state: green fill and border */
	.switch input:checked + .slider {
		background-color: #4ade80;
		border: 2px solid #2fba66;
	}
	.switch input:checked + .slider:before {
		transform: translateX(28px);
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
