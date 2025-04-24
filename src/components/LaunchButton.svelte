<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import LaunchAlertBox from "./LaunchAlertBox.svelte";
	import { addMessage } from "../lib/stores";
	import { onMount } from "svelte";

	let showAlert = false;
	let showDropdown = false;

	// Close dropdown when clicking outside
	onMount(() => {
		const handleClickOutside = (event: MouseEvent) => {
			const target = event.target as HTMLElement;
			if (!target.closest(".launch-container")) {
				showDropdown = false;
			}
		};

		document.addEventListener("click", handleClickOutside);

		return () => {
			document.removeEventListener("click", handleClickOutside);
		};
	});

	// Keep original toggleDropdown for mouse events
	const toggleDropdown = (event: MouseEvent) => {
		event.stopPropagation();
		showDropdown = !showDropdown;
	};

	// Add keyboard toggle for dropdown that doesn't conflict with MouseEvent
	const handleKeyToggle = (event: KeyboardEvent) => {
		if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			showDropdown = !showDropdown;
		}
	};

	const handleLaunch = async (mode: string) => {
		console.log(`Launching in ${mode} mode`);

		const path = await invoke("get_balatro_path");
		if (path && path.toString().includes("Steam")) {
			let is_balatro_running: boolean = await invoke(
				"check_balatro_running",
			);
			if (is_balatro_running) {
				addMessage("Balatro is already running", "error");
				return;
			}
			let is_steam_running: boolean = await invoke("check_steam_running");
			if (!is_steam_running) {
				showAlert = true;
				return;
			} else {
				await invoke("launch_balatro");
				return;
			}
		} else {
			await invoke("launch_balatro");
			return;
		}
	};

	const handleAlertClose = () => {
		showAlert = false;
	};
</script>

<div
	class="launch-container"
	onclick={(event) => event.stopPropagation()}
	onkeydown={(e) => e.key === "Escape" && (showDropdown = false)}
	role="button"
	tabindex="0"
>
	<button
		class="launch-button"
		onclick={toggleDropdown}
		onkeydown={handleKeyToggle}
		aria-haspopup="listbox"
		aria-expanded={showDropdown}
	>
		Launch
	</button>

	{#if showDropdown}
		<div class="dropdown-menu" role="listbox" tabindex="-1">
			<button
				class="dropdown-item"
				onclick={() => handleLaunch("Vanilla")}
				onkeydown={(e) =>
					(e.key === "Enter" || e.key === " ") &&
					handleLaunch("Vanilla")}
				role="option"
				aria-selected="false"
			>
				Vanilla
			</button>
			<button
				class="dropdown-item"
				onclick={() => handleLaunch("Modded")}
				onkeydown={(e) =>
					(e.key === "Enter" || e.key === " ") &&
					handleLaunch("Modded")}
				role="option"
				aria-selected="false"
			>
				Modded
			</button>
		</div>
	{/if}
</div>

<LaunchAlertBox show={showAlert} onClose={handleAlertClose} />

<style>
	.launch-container {
		position: absolute;
		top: 2.5rem;
		right: 0rem;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		z-index: 100;
	}

	.launch-button {
		background: #00a2ff;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 3.2rem;
		padding: 0.5rem 2.2rem;
		border: none;
		cursor: pointer;
		transition: all 0.2s ease;
		text-shadow:
			-2px -2px 0 #000,
			2px -2px 0 #000,
			-2px 2px 0 #000,
			2px 2px 0 #000;
		border-radius: 8px;
		outline: 3px solid #334461;
		box-shadow: inset 0 0 10px rgba(0, 0, 0, 0.3);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.launch-button:hover {
		background: #0088ff;
		transform: translateY(-2px);
	}

	.launch-button:active {
		transform: translateY(0);
	}

	.dropdown-menu {
		position: absolute;
		top: 100%;
		right: 0;
		margin-top: 0.5rem;
		background: #1c2832;
		border-radius: 8px;
		border: 3px solid #334461;
		overflow: hidden;
		width: 90%;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
	}

	.dropdown-item {
		background: none;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 2rem;
		padding: 0.6rem 1.5rem;
		width: 100%;
		text-align: center;
		border: none;
		cursor: pointer;
		transition: background 0.2s ease;
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000;
	}

	.dropdown-item:hover {
		background: #00a2ff;
	}

	.dropdown-item:not(:last-child) {
		border-bottom: 2px solid #334461;
	}

	@media (max-width: 1160px) {
		.launch-button {
			font-size: 2.8rem;
			text-shadow:
				-1.8px -1.8px 0 #000,
				1.8px -1.8px 0 #000,
				-1.8px 1.8px 0 #000,
				1.8px 1.8px 0 #000;
		}

		.launch-container {
			top: 2.4rem;
		}

		.dropdown-item {
			font-size: 1.6rem;
			padding: 0.5rem 1.2rem;
		}
	}
</style>
