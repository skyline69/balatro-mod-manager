<script lang="ts">
	import PathSelector from "../PathSelector.svelte";
	import { Settings2, RefreshCw } from "lucide-svelte";
	import { addMessage } from "$lib/stores";
	import { onMount } from "svelte";

	import { invoke } from "@tauri-apps/api/core";

	let isReindexing = false;
	let isClearingCache = false;
	let isConsoleEnabled = false;

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

	async function reindexMods() {
		isReindexing = true;
		try {
			await invoke("refresh_mods_folder");
			addMessage("Successfully re-indexed mods!", "success");
		} catch (error) {
			addMessage("Failed to re-index mods: " + error, "error");
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
	onMount(async () => {
		try {
			isConsoleEnabled = await invoke("get_lovely_console_status");
		} catch (error) {
			console.error("Failed to get console status:", error);
			addMessage("Error fetching Lovely Console status", "error");
		}
	});
</script>

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
				class="reindex-button"
				on:click={reindexMods}
				disabled={isReindexing}
			>
				{#if isReindexing}
					<div class="throbber"></div>
				{:else}
					<Settings2 size={20} />
					Reindex Mods
				{/if}
			</button>
			<p class="description">
				Removes any untracked mods from the Mods folder
			</p>
		</div>

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

<style>
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
	.description {
		color: #f4eee0;
		font-size: 1.2rem;
		margin-top: 0.5rem;
		opacity: 0.9;
		max-width: 400px;
		line-height: 1.4;
	} /* Custom Toggle Switch Styles */
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
		.clear-cache-button {
			font-size: 1rem;
			padding: 0.6rem 1.2rem;
		}
		.description {
			font-size: 1.1rem;
			max-width: 100%;
		}
	}
</style>
