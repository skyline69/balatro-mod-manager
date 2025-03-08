<script lang="ts">
	import { BookOpen, Coffee } from "lucide-svelte";
	import { open } from "@tauri-apps/plugin-shell";
	import { Confetti } from "svelte-confetti";

	let showConfetti = false;

	let buttonRect: DOMRect;

	const handleKofiClick = async (event: MouseEvent) => {
		const button = event.currentTarget as HTMLButtonElement;
		buttonRect = button.getBoundingClientRect();
		showConfetti = true;
		setTimeout(() => (showConfetti = false), 2000);
		try {
			await open("https://ko-fi.com/skyline69/goal?g=0");
		} catch (error) {
			console.error("Failed to open URL:", error);
		}
	};
</script>

<div class="container default-scrollbar">
	<div class="about-container">
		<div class="content">
			<h2>About Balatro Mod Manager</h2>
	
			<div>
				<h3>What is BMM?</h3>
				<p>
					Balatro Mod Manager (BMM) is a tool designed to help you manage
					and install mods for the game Balatro. It provides an
					easy-to-use interface for mod management while maintaining the
					game's unique aesthetic.
				</p>
			</div>
	
			<div>
				<h3>Features</h3>
				<ul>
					<li>Easy mod installation and management</li>
					<li>Automatic game path detection</li>
					<li>Mod compatibility checking</li>
					<li>Clean, pixel-perfect interface</li>
				</ul>
			</div>
	
			<div class="button-container">
				<button
					class="wiki-button"
					on:click={() =>
						open("https://balatromods.miraheze.org/wiki/Main_Page")}
				>
					<BookOpen size={20} />
					<span>Visit Wiki</span>
				</button>
				<button class="kofi-button" on:click={handleKofiClick}>
					<div class="confetti-container">
						{#if showConfetti}
							<Confetti
								x={[0, 1]}
								y={[0, 1]}
								duration={4000}
								amount={50}
							/>
						{/if}
					</div>
					<Coffee size={20} />
					<span>Support on Ko-fi</span>
				</button>
			</div>
	
			<p id="versiontext">Current version: v0.1.8</p>
		</div>
	
		<div class="profile-section">
			<div class="profile">
				<img src="/images/pb.jpg" alt="" />
			</div>
			<span class="profile-title">Efe/Skyline - The Creator of BMM</span>
		</div>
	</div>
</div>

<style>
	.about-container {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: center;

		padding: 0rem 2rem;

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

		&::-webkit-scrollbar:horizontal {
			display: none;
		}

		&::-webkit-scrollbar-corner {
			background-color: transparent;
		}
	}

	.profile-section {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1rem;
	}

	.profile {
		flex-shrink: 0;
		width: 200px;
		height: 200px;
		border-radius: 50%;
		overflow: hidden;
		border: 4px solid #f7f1e4;
		box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
	}

	.profile img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.content {
		flex: 1;
	}

	.profile-title {
		color: #f7f1e4;
		font-size: 1rem;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000;
	}

	.wiki-button {
		background-color: #fdcf51;
		border: 4px solid #f7f1e4;
		border-radius: 8px;
		color: #000;
		padding: 0.5rem 1rem;
		font-family: "M6X11", sans-serif;
		font-size: 1.2rem;
		cursor: pointer;
		transition: all 0.2s ease;
		box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		position: relative;
	}

	.wiki-button:hover {
		background-color: #fde700;
		transform: scale(1.05);
	}

	.wiki-button:active {
		transform: scale(0.95);
	}

	.kofi-button {
		background-color: #29abe0;
		border: 4px solid #f4eee0;
		border-radius: 8px;
		color: #fff;
		padding: 0.5rem 1rem;
		font-family: "M6X11", sans-serif;
		font-size: 1.2rem;
		cursor: pointer;
		transition: all 0.2s ease;
		box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		position: relative;
	}

	.kofi-button:hover {
		background-color: #13a3e1;
		transform: scale(1.05);
	}

	.kofi-button:active {
		transform: scale(0.95);
	}

	h2 {
		font-size: 2.5rem;
		margin-bottom: 1rem;
		color: #fdcf51;
	}

	h3 {
		font-size: 1.8rem;
		margin-bottom: 0.5rem;
		color: #fdcf51;
	}

	p {
		font-size: 1.2rem;
		margin-bottom: 1rem;
	}

	#versiontext {
		color: #fde700;
		margin-top: 1rem;
	}

	ul {
		list-style-type: disc;
		margin-left: 1rem;
		margin-bottom: 1rem;
	}

	li {
		font-size: 1.2rem;
		margin-bottom: 0.5rem;
	}
	.button-container {
		display: flex;
		gap: 1rem;
		margin: 1rem 0;
	}

	.confetti-container {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}

	@media (max-width: 1160px) {
		p {
			font-size: 1rem;
		}
		li {
			font-size: 1rem;
		}

		.profile {
			width: 170px;
			height: 170px;
		}
		.profile-title {
			font-size: 0.8rem;
		}

		h2 {
			font-size: 2rem;
		}
		h3 {
			font-size: 1.5rem;
		}
		.wiki-button {
			font-size: 1rem;
			padding: 0.4rem 0.8rem;
		}
		.kofi-button {
			font-size: 1rem;
			padding: 0.4rem 0.8rem;
		}
	}
</style>
