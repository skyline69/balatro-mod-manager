<script lang="ts">
	import { blur } from "svelte/transition";
	import MessageStack from "../components/MessageStack.svelte";
	import { backgroundEnabled } from "../stores/modStore"; // Add this import

	export let data;
</script>

<MessageStack />
<div
	class="layout-container"
	style:--gradient-opacity={$backgroundEnabled ? 0 : 1}
>
	{#key data.url}
		<div
			in:blur={{ duration: 300, delay: 150 }}
			out:blur={{ duration: 150 }}
			class="page-content"
		>
			<slot />
		</div>
	{/key}
</div>

<style>
	.layout-container {
		width: 100%;
		height: 100%;
		position: fixed;
		top: 0;
		left: 0;
	}

	.layout-container::before {
		content: "";
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		opacity: var(--gradient-opacity, 1);
		transition: opacity 0.4s cubic-bezier(0.4, 0, 0.2, 1);
		background: linear-gradient(
				145deg,
				rgba(97, 11, 15, 0.95) 0%,
				rgba(165, 25, 31, 0.9) 30%,
				rgba(194, 63, 55, 0.85) 70%,
				rgba(140, 20, 31, 0.95) 100%
			),
			linear-gradient(
				45deg,
				rgba(0, 0, 0, 0.15) 0%,
				rgba(50, 0, 0, 0.2) 100%
			);
		background-blend-mode: multiply;
		z-index: -1;
	}

	.page-content {
		width: 100%;
		height: 100%;
		position: relative;
	}
</style>
