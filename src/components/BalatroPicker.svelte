<script lang="ts">
	import { fly } from "svelte/transition";
	import { invoke } from "@tauri-apps/api/core";
	import { addMessage } from "../lib/stores";

	import { goto } from "$app/navigation";

	let selectedOption = "steam";
	let showCustomInput = false;
	let selectedPath = "";

	const handleOptionChange = (option: string) => {
		selectedOption = option;
		showCustomInput = option === "custom";

		if (option == "custom") {
			selectedPath = "";
		}
	};

	const truncatePath = (path: string) => {
		const maxLength = 50;
		return path.length > maxLength
			? path.substring(0, maxLength - 3) + "..."
			: path;
	};

	const handlePathSelect = async () => {
		const { open } = await import("@tauri-apps/plugin-dialog");

		const selected = await open({
			directory: true,
			multiple: false,
			title: "Select Balatro Path",
		});
		if (selected) {
			selectedPath = selected as string;
		}
	};

	let isLoading = false;

	const handleClick = async () => {
		isLoading = true;
		try {
			if (selectedOption === "steam") {
				const paths: string[] = await invoke("find_steam_balatro");
				if (paths.length === 0) {
					addMessage(
						"Balatro not found in Steam installation",
						"error",
					);
				} else {
					selectedPath = paths[0];
					addMessage(
						"Successfully found Balatro installation",
						"success",
					);

					await new Promise((resolve) => setTimeout(resolve, 1000));
					await goto("/main", { replaceState: true });
				}
			} else if (selectedOption === "custom") {
				if (!selectedPath) {
					addMessage("Please select a custom path", "warning");
					isLoading = false;
					return;
				}
				const isValid = await invoke("check_custom_balatro", {
					path: selectedPath,
				});
				if (isValid) {
					addMessage(
						"Successfully found Balatro installation",
						"success",
					);
					// Wait for the success message to show
					await new Promise((resolve) => setTimeout(resolve, 1000));
					await goto("/main", { replaceState: true });
				} else {
					addMessage("Invalid Balatro path", "error");
				}
			}
		} catch (error) {
			addMessage("Error finding Balatro: " + error, "error");
		} finally {
			isLoading = false;
		}
	};
</script>

<div class="page-wrapper">
	<div class="container" class:expanded={showCustomInput}>
		<h2>Where is Balatro?</h2>
		<p>Select Balatro Path</p>

		<div class="radio-group">
			<label class="radio-label">
				<input
					type="radio"
					name="location"
					value="steam"
					disabled={isLoading}
					bind:group={selectedOption}
					on:change={() => handleOptionChange("steam")}
				/>
				<span class="radio-text">Steam</span>
			</label>

			<label class="radio-label">
				<input
					type="radio"
					name="location"
					value="custom"
					disabled={isLoading}
					bind:group={selectedOption}
					on:change={() => handleOptionChange("custom")}
				/>
				<span class="radio-text">Custom</span>
			</label>
		</div>

		{#if showCustomInput}
			<div
				class="input-container"
				in:fly={{ duration: 200, y: 10 }}
				out:fly={{ duration: 200, y: -10 }}
			>
				<input
					type="text"
					placeholder="Choose Balatro Path"
					disabled={isLoading}
					on:click={handlePathSelect}
					value={selectedPath ? truncatePath(selectedPath) : ""}
					readonly
				/>
			</div>
		{/if}

		<div class="button-wrapper">
			{#if isLoading}
				<div class="overlay"></div>
			{/if}
			<button
				class="action-button"
				on:click={handleClick}
				disabled={isLoading}
			>
				{#if isLoading}
					<div class="throbber"></div>
				{:else}
					Continue
				{/if}
			</button>
		</div>
	</div>
</div>

<style>
	:root {
		/* Base Colors */
		/* --color-dark: #459373; */
		/* --color-medium: #56a786; */
		/* --color-light: #74cca8; */
		/* --color-cream: #f4eee0; */
		--color-dark: #3b41a8; /* Brighter base blue */
		--color-medium: #4b52d1; /* Vibrant medium blue */
		--color-light: #6166ff; /* Bright highlight blue */
		--color-cream: #f4eee0; /* Original cream color */

		--text-primary: var(--color-cream);
		--text-secondary: var(--color-light);
		--background-primary: var(--color-dark);
		--background-secondary: var(--color-medium);
		--accent: var(--color-light);
	}

	.page-wrapper {
		display: flex;
		justify-content: center;
		align-items: center;
		width: 100%;
		overflow-x: hidden; /* Prevent horizontal scrolling */
	}

	.button-wrapper {
		position: relative;
	}

	.overlay {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: transparent;
		z-index: 100;
	}

	.container {
		display: flex;
		flex-direction: column;
		align-items: center;
		width: 400px;
		max-width: 95vw; /* Ensure it doesn't overflow on smaller screens */
		padding: 2.5rem;
		border-radius: 25px;
		background-color: var(--background-primary);
		outline: 2px solid var(--color-medium);
		color: var(--text-primary);
		transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
		height: auto;
		min-height: 150px;
		position: relative;
		box-sizing: border-box; /* Include padding in width calculation */
	}

	.action-button {
		margin-top: 2rem;
		padding: 0.75rem 2rem;
		border: 2px solid var(--color-cream);
		border-radius: 12px;
		background-color: transparent;
		color: var(--color-cream);
		font-family: inherit;
		font-size: 1.4rem; /* Slightly larger font */
		cursor: pointer;
		width: 150px; /* Increased from 120px */
		height: 60px; /* Increased from 45px */
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
		overflow: hidden; /* Prevents content from spilling during transition */
	}
	.action-button:hover {
		background-color: var(--color-cream);
		color: var(--background-primary);
	}

	.action-button:active {
		transform: scale(0.98);
	}

	.action-button:disabled {
		cursor: not-allowed;
		opacity: 0.8;
		background-color: var(--color-medium);
		transform: none;
		transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.action-button:has(.throbber) {
		width: 70px; /* Smaller width when showing throbber */
		padding: 0.75rem;
		background-color: #f4eee0;
	}

	.throbber {
		width: 24px;
		height: 24px;
		border: 3px solid var(--background-primary);
		border-radius: 50%;
		border-top-color: transparent;
		animation:
			spin 1s linear infinite,
			fade-in 0.3s ease;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@keyframes fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	.container.expanded {
		min-height: 250px;
	}

	.input-container {
		width: 100%;
		margin: 1rem 0; /* Change from 1rem to 1rem 0 to prevent horizontal overflow */
		box-sizing: border-box; /* Include padding in width calculation */
	}

	h2 {
		margin: 0;
		font-size: 2rem;
		font-weight: 600;
		text-align: center;
	}

	p {
		margin: 0.5rem 0 1.5rem;
		font-size: 1.2rem;
		opacity: 0.9;
		text-align: center;
	}

	.radio-group {
		display: flex;
		flex-direction: column;
		gap: 1rem; /* Increased from 1rem to 1.5rem to match the spacing in the image */
		width: 100%;
	}
	.radio-label {
		display: flex;
		align-items: center;
		gap: 1rem;
		cursor: pointer;
	}

	.radio-label input[type="radio"] {
		appearance: none;
		-webkit-appearance: none;
		width: 24px;
		height: 24px;
		border: 2px solid var(--color-cream);
		border-radius: 50%;
		margin: 0;
		cursor: pointer;
		position: relative;
		transition: all 0.2s ease;
	}

	.radio-label input[type="radio"]:checked {
		border-color: var(--color-cream);
		background-color: transparent;
	}

	.radio-label input[type="radio"]:checked::after {
		content: "";
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background-color: var(--color-cream);
	}

	.radio-label input[type="radio"]:hover {
		border-color: var(--color-light);
		opacity: 0.8;
	}

	.radio-text {
		color: #f4eee0;
		/* distance between 2 elements */
		font-size: 1.4rem;
	}

	.input-container {
		width: 100%;
		margin: 1rem;
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
		box-sizing: border-box; /* Include padding and border in width calculation */
	}

	input[type="text"]:hover {
		border-color: var(--text-primary);
	}

	input[type="text"]::placeholder {
		color: white;
		-webkit-user-select: none;
		user-select: none;
	}
</style>
