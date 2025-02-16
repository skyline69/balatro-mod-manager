<script lang="ts">
	import { fade, scale } from "svelte/transition";
    import { showWarningPopup } from "../stores/modStore";

	export let visible: boolean = false;
	export let message: string = "";
	export let onConfirm: () => void;
	export let onCancel: () => void;

	function handleConfirm() {
		onConfirm();
		showWarningPopup.update((p) => ({ ...p, visible: false }));
	}

	function handleCancel() {
		onCancel();
		showWarningPopup.update((p) => ({ ...p, visible: false }));
	}
</script>

{#if visible}
	<div class="modal-background" transition:fade={{ duration: 100 }}>
		<div
			class="modal"
			transition:scale={{ duration: 200, start: 0.95, opacity: 1 }}
		>
			<h2>Warning</h2>
			<p>{message}</p>
			<div class="buttons">
				<button class="cancel-button" on:click={handleCancel}
					>Cancel</button
				>
				<button class="confirm-button" on:click={handleConfirm}
					>Confirm</button
				>
			</div>
		</div>
	</div>
{/if}

<style>
	.modal-background {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 999;
	}
	.modal {
		background: #2d2d2d;
		outline: 2px solid #f87171;
		padding: 2rem;
		border-radius: 8px;
		box-shadow: 0 0 15px rgba(0, 0, 0, 0.7);
		max-width: 500px;
		width: 90%;
		text-align: center;
	}
	h2 {
		color: #f87171;
		margin-bottom: 1rem;
		font-family: "M6X11", sans-serif;
	}
	p {
		color: #f4eee0;
		font-size: 1.2rem;
		margin-bottom: 2rem;
		font-family: "M6X11", sans-serif;
	}
	.buttons {
		display: flex;
		justify-content: center;
		gap: 1rem;
	}
	button {
		padding: 0.8rem 1.5rem;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		font-size: 1rem;
		transition: all 0.2s ease;
		font-family: "M6X11", sans-serif;
	}
	.cancel-button {
		background: #f87171;
		outline: #f0a7a7 solid 2px;
		color: #fff;
	}
	.confirm-button {
		background: #56a786;
		outline: #74cca8 solid 2px;
		color: #fff;
	}
	button:hover {
		opacity: 0.9;
		scale: 1.05;
		transition: all 0.2s ease;
	}
</style>
