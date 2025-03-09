<script lang="ts">
	import BalatroPicker from "../components/BalatroPicker.svelte";
	import { Menu, MenuItem } from "@tauri-apps/api/menu";
	import { onMount } from "svelte";
	import { invoke } from "@tauri-apps/api/core";
	import { goto } from "$app/navigation";

	let starsContainer: HTMLElement;

	onMount(() => {
		const init = async () => {
			try {
				const existingPath = await invoke(
					"check_existing_installation",
				);
				if (existingPath) {
					await goto("/main", { replaceState: true });
				}
				createStars();
			} catch (error) {
				console.error("Error checking existing installation:", error);
			}
		};

		init();

		window.addEventListener("resize", createStars);

		return () => window.removeEventListener("resize", createStars);
	});

	function createStars() {
		if (!starsContainer) return;
		starsContainer.innerHTML = "";

		const numberOfStars = 150;
		const colors = ["#ffffff", "#fffacd", "#87ceeb"];

		// Create normal stars
		for (let i = 0; i < numberOfStars; i++) {
			const star = document.createElement("div");
			star.className = "star";

			star.style.left = `${Math.random() * 100}%`;
			star.style.top = `${Math.random() * 100}%`;

			const size = Math.random() * 2 + 1;
			star.style.width = `${size}px`;
			star.style.height = `${size}px`;

			star.style.backgroundColor =
				colors[Math.floor(Math.random() * colors.length)];

			const duration = 2 + Math.random() * 3;
			const delay = Math.random() * 2;
			star.style.animation = `twinkle ${duration}s infinite ${delay}s`;

			starsContainer.appendChild(star);
		}

		// Create shooting stars [1]
		const numberOfShootingStars = 5;
		for (let i = 0; i < numberOfShootingStars; i++) {
			const shootingStar = document.createElement("div");
			shootingStar.className = "shooting-star";

			shootingStar.style.left = `${Math.random() * 100}%`;
			shootingStar.style.top = `${Math.random() * 50}%`;

			const duration = 1 + Math.random() * 2;
			shootingStar.style.animation = `shoot ${duration}s linear infinite`;

			starsContainer.appendChild(shootingStar);
		}
	}

	window.addEventListener("contextmenu", async (e) => {
		e.preventDefault();
		const menuItems = [
			await MenuItem.new({
				text: "Copy",
				action: () => {},
			}),
			await MenuItem.new({
				text: "Paste",
				action: () => {},
			}),
		];
		const menu = await Menu.new({ items: menuItems });
		menu.popup();
	});
</script>

<div class="night-sky">
	<div class="stars" bind:this={starsContainer}></div>
</div>

<div class="app">
	<h1>Welcome to Balatro Mod Manager</h1>
	<BalatroPicker />
	<div class="version-text">v0.1.9</div>
</div>

<style>
	.night-sky {
		width: 100vw;
		height: 100vh;
		background: linear-gradient(
			to bottom,
			#0f1016 0%,
			#1a1b2e 50%,
			#232b4e 100%
		);
		position: fixed;
		top: 0;
		left: 0;
		z-index: -1;
		overflow: hidden;
	}

	.stars {
		width: 100%;
		height: 100%;
		position: fixed;
		top: 0;
		left: 0;
	}

	:global(.star) {
		position: absolute;
		border-radius: 50%;
		pointer-events: none;
		box-shadow:
			0 0 4px currentColor,
			0 0 8px currentColor;
		will-change: transform, opacity;
		transition: all 0.3s ease;
	}

	@keyframes twinkle {
		0% {
			opacity: 0.2;
			transform: scale(0.8);
			box-shadow:
				0 0 2px currentColor,
				0 0 4px currentColor;
		}
		50% {
			opacity: 1;
			transform: scale(1.2);
			box-shadow:
				0 0 6px currentColor,
				0 0 12px currentColor,
				0 0 18px currentColor;
		}
		100% {
			opacity: 0.2;
			transform: scale(0.8);
			box-shadow:
				0 0 2px currentColor,
				0 0 4px currentColor;
		}
	}

	/* Shooting stars [1] */
	:global(.shooting-star) {
		position: absolute;
		width: 2px;
		height: 2px;
		background-color: #ffffff;
		box-shadow: 0 0 6px #ffffff;
		border-radius: 50%;
		transform: translateX(-50%) translateY(-50%);
		animation: shoot 2s linear infinite;
	}

	@keyframes shoot {
		0% {
			opacity: 1;
			transform: translateX(0) translateY(0);
		}
		100% {
			opacity: 0;
			transform: translateX(100px) translateY(100px);
		}
	}

	.app {
		width: 100vw;
		height: 100vh;
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 2rem;
		box-sizing: border-box;
		overflow: hidden;
		position: fixed;
		top: 0;
		left: 0;
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
	}

	h1 {
		color: #f4eee0;
		font-size: 3.5rem;
		margin-bottom: 3rem;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-2px -2px 0 #000,
			2px -2px 0 #000,
			-2px 2px 0 #000,
			2px 2px 0 #000;
	}

	:global(html) {
		overflow: hidden;
		position: fixed;
		width: 100%;
		height: 100%;
	}

	:root {
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		line-height: 24px;
		font-weight: 400;
		color: var(--text-primary);
		font-synthesis: none;
		text-rendering: optimizeLegibility;
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
		-webkit-text-size-adjust: 100%;
		-webkit-user-select: none;
		user-select: none;
		cursor: default;
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000;
	}

	.version-text {
		position: fixed;
		bottom: 1rem;
		right: 1rem;
		color: #f4eee0;
		font-size: 1rem;
		opacity: 0.8;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000;
	}

	:global(body) {
		margin: 0;
		padding: 0;
		overflow: hidden;
		position: fixed;
		width: 100%;
		height: 100%;
	}
</style>
