<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { FolderDot } from "lucide-svelte";
	import MessageStack from "./MessageStack.svelte";
    import { addMessage } from "$lib/stores";

	// let messageStack: MessageStack;
	let selectedPath = "";
	let placeholder = "Choose Balatro Path";
	let isLoading = false;

	const truncatePath = (path: string) => {
		const maxLength = 50;
		return path.length > maxLength
			? path.substring(0, maxLength - 3) + "..."
			: path;
	};

	const getBalatroPath = async () => {
		const path = await invoke("get_balatro_path");
		if (path) {
			selectedPath = path as string;
			placeholder = path as string;
		} else {
			placeholder = "Choose Balatro Path";
		}
	};

	getBalatroPath();

	const handlePathSelect = async () => {
		const { open } = await import("@tauri-apps/plugin-dialog");
		const selected = await open({
			directory: true,
			multiple: false,
			title: "Select Balatro Path",
		});

		if (selected) {
			selectedPath = selected as string;
			const isValid = await invoke("check_custom_balatro", {
				path: selectedPath,
			});

			if (isValid) {
				await invoke("set_balatro_path", { path: selectedPath });
				addMessage(
					"Balatro path set successfully!",
					"success",
				);
			} else {
				addMessage(
					"Invalid Balatro path. Please select the correct directory.",
					"error",
				);
				selectedPath = "";
			}
		}
	};

	const setSteamPath = async () => {
		isLoading = true;
		try {
			const paths: string[] = await invoke("find_steam_balatro");
			if (paths.length === 0) {
				addMessage(
					"Balatro not found in Steam installation",
					"error",
				);
			} else {
				selectedPath = paths[0];
				await invoke("set_balatro_path", { path: selectedPath });
				addMessage(
					"Successfully set Steam path",
					"success",
				);
			}
		} catch (error) {
			addMessage(
				"Error finding Steam path: " + error,
				"error",
			);
		} finally {
			isLoading = false;
		}
	};
</script>

<div class="path-selector">
	<div class="input-container">
		<input
			type="text"
			placeholder={truncatePath(placeholder)}
			value={selectedPath ? truncatePath(selectedPath) : ""}
			on:click={handlePathSelect}
			readonly
			disabled={isLoading}
		/>
	</div>
	<button class="steam-button" on:click={setSteamPath} disabled={isLoading}>
		{#if isLoading}
			<div class="throbber"></div>
		{:else}
			<FolderDot size={20} />
			Set to Steam Path
		{/if}
	</button>
</div>

<!-- <MessageStack bind:this={messageStack} /> -->

<style>
	.path-selector {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin: 1rem 0;
	}

	.input-container {
		width: 20rem;
	}

	input[type="text"] {
		width: 100%;
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

	.steam-button {
		padding: 0.5rem 1rem;
		border: 2px solid #2e6b9a;
		border-radius: 8px;
		background-color: #1a9fff;
		color: #ffffff;
		font-family: inherit;
		font-size: 1.1rem;
		cursor: pointer;
		width: 13rem;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		transition: all 0.2s ease;
		overflow: hidden;
	}

	.steam-button:hover {
		background-color: #3daeff;
		border: 2px solid #8fc8ff;
	}

	.steam-button:active {
		transform: scale(0.98);
	}

	.steam-button:disabled {
		cursor: not-allowed;
		opacity: 0.8;
		background-color: #7fc8ff;
		transform: none;
	}
	.throbber {
		width: 20px;
		height: 20px;
		border: 3px solid #f4eee0;
		border-radius: 50%;
		border-top-color: transparent;
		animation: spin 1s linear infinite;
	}

	@media (max-width: 1160px) {
		.steam-button {
			width: 12rem;
			font-size: 1rem;
		}
		.steam-button :global(svg) {
			scale: 0.8;
		}
		.steam-button .throbber {
			width: 15px;
			height: 15px;
		}
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
