<script lang="ts">
	import { fly } from "svelte/transition";
	import { invoke } from "@tauri-apps/api/core";
	import { addMessage } from "../lib/stores";
	import { FolderDot, FileDigit } from "lucide-svelte";
	import { goto } from "$app/navigation";

	let selectedOption = "steam";
	let showCustomInput = false;
	let selectedPath = "";
	let customPathType = "directory"; // New state to track directory or executable selection

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
			directory: customPathType === "directory",
			multiple: false,
			title:
				customPathType === "directory"
					? "Select Balatro Directory"
					: "Select Balatro Executable",
			filters:
				customPathType === "executable"
					? [{ name: "Executable", extensions: ["exe"] }]
					: undefined,
		});

		if (selected) {
			selectedPath = selected as string;
		}
	};

	let isLoading = false;

	const handleClick = async () => {
		isLoading = true;
		try {
			async function safeNavigateToMain() {
				const target = "/main";
				try {
					await goto(target, { replaceState: true });
				} catch (_) {
					// ignore and try hard navigation below
				}
				// Give the router a brief moment to update location
				await new Promise((r) => setTimeout(r, 150));
				if (window.location.pathname.endsWith("/main")) return;
				// Fallback for older WebKit on macOS (Monterey)
				try {
					window.location.href = "/main.html";
				} catch (_) {
					window.location.href = target;
				}
			}

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
					await safeNavigateToMain();
				}
			} else if (selectedOption === "custom") {
				if (!selectedPath) {
					addMessage("Please select a custom path", "warning");
					isLoading = false;
					return;
				}

				// If executable was selected, get the parent directory
				let pathToCheck = selectedPath;
				if (customPathType === "executable") {
					// Extract directory from file path
					const lastSlashIndex =
						selectedPath.lastIndexOf("/") !== -1
							? selectedPath.lastIndexOf("/")
							: selectedPath.lastIndexOf("\\");

					if (lastSlashIndex !== -1) {
						pathToCheck = selectedPath.substring(0, lastSlashIndex);
					}
				}

				const isValid = await invoke("check_custom_balatro", {
					path: pathToCheck,
				});

				if (isValid) {
					addMessage(
						"Successfully found Balatro installation",
						"success",
					);
					// Wait for the success message to show
					await new Promise((resolve) => setTimeout(resolve, 1000));
					await safeNavigateToMain();
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
				class="custom-input-section"
				in:fly={{ duration: 200, y: 10 }}
				out:fly={{ duration: 200, y: -10 }}
			>
				<div class="path-type-selector">
					<button
						class:active={customPathType === "directory"}
						on:click={() => (customPathType = "directory")}
						disabled={isLoading}
					>
						<FolderDot size={16} />
						Directory
					</button>
					<button
						class:active={customPathType === "executable"}
						on:click={() => (customPathType = "executable")}
						disabled={isLoading}
					>
						<FileDigit size={16} />
						Executable
					</button>
				</div>

				<div class="input-container">
					<input
						type="text"
						placeholder={customPathType === "directory"
							? "Choose Balatro Directory"
							: "Choose Balatro Executable"}
						disabled={isLoading}
						on:click={handlePathSelect}
						value={selectedPath ? truncatePath(selectedPath) : ""}
						readonly
					/>
				</div>
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
		margin-top: auto; /* Push to bottom of flex container */
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
		margin-top: 1rem; /* Reduced from 2rem */
		padding: 0.75rem 2rem;
		border: 2px solid var(--color-cream);
		border-radius: 12px;
		background-color: transparent;
		color: var(--color-cream);
		font-family: inherit;
		font-size: 1.4rem;
		cursor: pointer;
		width: 150px;
		height: 60px;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
		overflow: hidden;
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
		min-height: 290px; /* Reduced from 320px */
		display: flex;
		flex-direction: column;
		justify-content: space-between; /* Distribute content evenly */
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
		gap: 1rem;
		width: 100%;
		margin-bottom: 1.5rem; /* Added to reduce space after radio buttons */
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
		font-size: 1.4rem;
	}

	.custom-input-section {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 0.25rem; /* Reduced from 0.5rem */
	}

	.path-type-selector {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.25rem; /* Reduced from 0.5rem */
		width: 100%;
	}

	.path-type-selector button {
		flex: 1;
		background: #c88000;
		color: white;
		border: 2px solid #fda200;
		border-radius: 8px;
		padding: 0.5rem;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		cursor: pointer;
		transition: all 0.2s ease;
		font-family: "M6X11", sans-serif;
		font-size: 0.9rem;
	}

	.path-type-selector button.active {
		background: #fda200;
		border-color: white;
	}

	.path-type-selector button:hover:not(:disabled) {
		border-color: white;
	}

	.path-type-selector button:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.input-container {
		width: 100%;
		margin: 0.25rem 0; /* Reduced from 0.5rem */
		box-sizing: border-box;
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
		box-sizing: border-box;
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
