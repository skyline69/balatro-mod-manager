<script lang="ts">
	import { fly } from "svelte/transition";
	import { messageStore } from "../lib/stores";
</script>

<div class="message-stack">
	{#each $messageStore as message (message.id)}
		<div
			class="message {message.type}"
			transition:fly={{ y: -20, duration: 200 }}
		>
			{message.text}
		</div>
	{/each}
</div>

<style>
	.message-stack {
		position: fixed;
		top: 20px;
		right: 20px;
		z-index: 9999;
		display: flex;
		flex-direction: column;
		gap: 10px;
		pointer-events: none;
	}

	.message {
		padding: 12px 20px;
		border-radius: 8px;
		color: white;
		font-weight: 500;
		box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
	}

	.success {
		background-color: #459373;
		border: 2px solid white;
	}

	.error {
		background-color: #644047;
		border: 2px solid #a25b5b;
	}

	.info {
		background-color: #435662;
		border: 2px solid #587a93;
	}

	.warning {
		background-color: #625c43;
		border: 2px solid #a39358;
	}

	.icon {
		font-size: 1.2rem;
		font-weight: bold;
	}
</style>
