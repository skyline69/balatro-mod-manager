<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import LaunchAlertBox from "./LaunchAlertBox.svelte";
	import { addMessage } from "../lib/stores";

	let showAlert = false;

	const handleLaunch = async () => {
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

<!-- <MessageStack bind:this={messageStack} /> -->

<div class="launch-container">
	<button class="launch-button" on:click={handleLaunch}> Launch </button>
</div>

<LaunchAlertBox show={showAlert} onClose={handleAlertClose} />

<style>
	.launch-container {
		position: absolute;
		top: 2.5rem;
		right: 0rem;
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
	}

	.launch-button:hover {
		background: #0088ff;
		transform: translateY(-2px);
	}

	.launch-button:active {
		transform: translateY(0);
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
	}
</style>
