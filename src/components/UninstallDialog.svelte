<script lang="ts">
	import { fade, scale } from "svelte/transition";
	import { invoke } from "@tauri-apps/api/core";
	import { createEventDispatcher } from "svelte";
	import { uninstallDialogStore } from "../stores/modStore";

	const dispatch = createEventDispatcher();

	export let show: boolean = false;
	export let modName: string;
	export let dependents: string[];
	export let modPath: string;

	let action: "cancel" | "force" | "cascade" | null = null;

	uninstallDialogStore.subscribe((state) => {
		show = state.show;
		modName = state.modName;
		modPath = state.modPath;
		dependents = state.dependents;
	});

	async function handleUninstall() {
		try {
			let success = false;

			if (action === "cascade") {
				await invoke("cascade_uninstall", { rootMod: modName });
				success = true;
			} else if (action === "force") {
				await invoke("force_remove_mod", {
					name: modName,
					path: modPath,
				});
				success = true;
			} else if (action === null) {
				await invoke("remove_installed_mod", {
					name: modName,
					path: modPath,
				});
				success = true;
			}

			if (success) {
				dispatch("uninstalled", {
					modName,
					success: true,
					action: action || "single",
				});
			}
		} catch (e) {
			dispatch("error", e);
		} finally {
			uninstallDialogStore.update((s) => ({ ...s, show: false }));
		}
	}
	function closeDialog() {
		uninstallDialogStore.update((s) => ({ ...s, show: false }));
	}
	$: isCoreMod =
		modName?.toLowerCase() === "steamodded" ||
		modName?.toLowerCase() === "talisman";
</script>

{#if show && isCoreMod}
	<div class="dialog-overlay" transition:fade={{ duration: 100 }}>
		<div class="dialog-content" transition:scale={{ duration: 200 }}>
			<h2>Uninstall {modName}?</h2>

			{#if dependents.length > 0}
				<div class="dependency-list">
					<h3>{modName} is required for:</h3>
					<div class="scroll-container">
						<ul>
							{#each dependents as dependent}
								<li>{dependent}</li>
							{/each}
						</ul>
					</div>
				</div>

				<div class="actions">
					<button
						class="confirm-button"
						on:click={() => {
							action = "cascade";
							handleUninstall();
						}}
					>
						Uninstall All ({dependents.length})
					</button>
					<button
						class="force-button"
						on:click={() => {
							action = "force";
							handleUninstall();
						}}
					>
						Force Remove Anyway
					</button>
					<button class="cancel-button" on:click={closeDialog}>
						Cancel
					</button>
				</div>
			{:else}
				<div class="actions">
					<button
						class="confirm-button"
						on:click={() => {
							action = null;
							handleUninstall();
						}}>Confirm</button
					>
					<button on:click={closeDialog} class="force-button"
						>Cancel</button
					>
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.dependency-list {
		margin: 1rem 0;
		padding: 1rem;
		background: rgba(244, 238, 224, 0.1);
		border-radius: 6px;
	}

	.dependency-list h3 {
		color: #fdcf51;
		margin-bottom: 0.5rem;
		font-size: 1.5rem;
	}

	.dependency-list ul {
		list-style-type: square;
		padding-left: 1.5rem;
	}

	.dependency-list li {
		color: #f4eee0;
		margin-bottom: 0.25rem;
	}

	.scroll-container {
		max-height: 40vh;
		overflow-y: auto;
		margin: 0.5rem 0;
		padding-right: 0.5rem;

		&::-webkit-scrollbar {
			width: 10px;
		}

		&::-webkit-scrollbar-track {
			background: transparent;
			border-radius: 15px;
		}

		&::-webkit-scrollbar-thumb {
			background: #f4eee0;
			border: 2px solid rgba(193, 65, 57, 0.8);
			border-radius: 15px;
		}
	}

	.dialog-overlay {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: rgba(0, 0, 0, 0.8);
		display: flex;
		justify-content: center;
		align-items: center;
		z-index: 1000;
		backdrop-filter: blur(2px);
	}

	.dialog-content {
		background: #393646;
		border: 2px solid #f4eee0;
		border-radius: 12px;
		padding: 2rem;
		width: 560px;
		max-width: 90%;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}

	.dialog-content h2 {
		text-align: center;
	}

	h2 {
		color: #fdcf51;
		font-size: 1.8rem;
		margin: 0 0 1.5rem;
		font-family: "M6X11", sans-serif;
	}

	ul {
		list-style: none;
		padding: 0;
		margin: 0;
	}

	li {
		color: #c14139;
		padding: 0.5rem 0;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 1.2rem;
	}

	li::before {
		content: "⚠️";
		font-size: 0.9rem;
	}

	.actions {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
		gap: 1rem;
		margin-top: 2rem;
	}

	button {
		padding: 1rem 1.5rem;
		border: none;
		border-radius: 6px;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		cursor: pointer;
		transition:
			transform 0.2s ease,
			background-color 0.2s ease;
	}

	.confirm-button {
		background: #56a786; /* Green from your download button */
		color: #f4eee0;
		border: 2px solid #459373;
	}

	.confirm-button:hover {
		background: #67b897;
		transform: translateY(-2px);
	}

	.force-button {
		background: #c14139; /* Red from delete button */
		color: #f4eee0;
		border: 2px solid #a13029;
	}

	.force-button:hover {
		background: #d2524a;
		transform: translateY(-2px);
	}

	.cancel-button {
		background: #ea9600; /* Orange from categories button */
		color: #f4eee0;
		border: 2px solid #cc8400;
	}

	.cancel-button:hover {
		background: #fca800;
		transform: translateY(-2px);
	}
	.cancel-button:active,
	.force-button:active,
	.confirm-button:active {
		transform: translateY(0);
	}

	/* Update the actions grid */
	.actions {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
		gap: 1rem;
		margin-top: 2rem;
	}

	@media (max-width: 768px) {
		.dialog-content {
			padding: 1.5rem;
			width: 90%;
			margin: 1rem;
		}

		.scroll-container {
			max-height: 35vh;
		}

		h2 {
			font-size: 1.5rem;
			margin-bottom: 1rem;
		}

		.dependency-list {
			padding: 0.75rem;
			margin: 0.75rem 0;
		}

		.dependency-list h3 {
			font-size: 1.2rem;
		}

		.dependency-list li {
			font-size: 1rem;
			padding: 0.25rem 0;
		}

		button {
			padding: 0.75rem 1rem;
			font-size: 0.95rem;
		}

		.actions {
			grid-template-columns: 1fr;
			gap: 0.75rem;
			margin-top: 1.5rem;
		}
	}

	@media (max-width: 480px) {
		.dialog-content {
			padding: 1rem;
			border-width: 1px;
		}

		.scroll-container {
			max-height: 30vh;
		}

		h2 {
			font-size: 1.3rem;
			margin-bottom: 0.75rem;
		}

		.dependency-list h3 {
			font-size: 1.1rem;
		}

		.dependency-list li {
			font-size: 0.9rem;
			gap: 0.5rem;
		}

		button {
			padding: 0.6rem 0.8rem;
			font-size: 0.9rem;
		}

		.actions {
			gap: 0.5rem;
			margin-top: 1rem;
		}
	}

	@media (max-width: 360px) {
		.dialog-content {
			padding: 0.75rem;
		}

		h2 {
			font-size: 1.2rem;
		}

		button {
			font-size: 0.85rem;
			padding: 0.5rem 0.7rem;
		}
	}
</style>
