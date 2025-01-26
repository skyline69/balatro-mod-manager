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
			<div class="message-content">
				{#if message.type === "success"}
					<svg class="icon" viewBox="0 0 24 24">
						<path
							fill="#74CCA8"
							d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z"
						/>
					</svg>
				{:else if message.type === "error"}
					<svg class="icon" viewBox="0 0 24 24">
						<path
							fill="#A25B5B"
							d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z"
						/>
					</svg>
				{:else if message.type === "info"}
					<svg class="icon" viewBox="0 0 24 24">
						<path
							fill="#587A93"
							d="M13,9H11V7H13M13,17H11V11H13M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2Z"
						/>
					</svg>
				{:else if message.type === "warning"}
					<svg class="icon" viewBox="0 0 24 24">
						<path
							fill="#A39358"
							d="M12,2L1,21H23M12,6L19.53,19H4.47M11,10V14H13V10M11,16V18H13V16"
						/>
					</svg>
				{/if}
				<span class="text">{message.text}</span>
			</div>
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
		gap: 12px;
		pointer-events: none;
	}

	.message {
		padding: 16px 24px;
		border-radius: 8px;
		color: white;
		font-weight: 500;
		box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
		min-width: 300px;
	}

	.message-content {
		display: flex;
		align-items: center;
		gap: 14px;
	}

	.icon {
		width: 28px;
		height: 28px;
		flex-shrink: 0;
		color: rgba(255, 255, 255, 0.9);
		filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.2));
	}

	.text {
		font-size: 1.1rem;
		line-height: 1.4;
		flex-grow: 1;
	}

	/* Updated background colors for better contrast */
	.success {
		background-color: #2d6b4f;
		border: 2px solid #459373;
	}

	.error {
		background-color: #4a2d32;
		border: 2px solid #644047;
	}

	.info {
		background-color: #2d3d4a;
		border: 2px solid #435662;
	}

	.warning {
		background-color: #4a452d;
		border: 2px solid #625c43;
	}
	@media (max-width: 1160px) {
		.message-stack {
			top: 15px;
			right: 15px;
			left: auto;
		}

		.message {
			min-width: unset;
			width: 90%;
			max-width: 320px;
			padding: 12px 16px;
			border-radius: 6px;
		}

		.message-content {
			gap: 10px;
		}

		.icon {
			width: 24px;
			height: 24px;
		}

		.text {
			font-size: 1rem;
			line-height: 1.35;
			word-break: break-word;
		}

		.success,
		.error,
		.info,
		.warning {
			border-width: 1.5px;
		}
	}
</style>
