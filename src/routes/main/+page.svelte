<script lang="ts">
	import ShaderBackground from "../../components/ShaderBackground.svelte";
	import About from "../../components/viewblock/About.svelte";
	import LaunchButton from "../../components/LaunchButton.svelte";
	import Mods from "../../components/viewblock/Mods.svelte";
	import Settings from "../../components/viewblock/Settings.svelte";
	import RequiresPopup from "../../components/RequiresPopup.svelte";
	import WarningPopup from "../../components/WarningPopup.svelte";
	import type { DependencyCheck, InstalledMod } from "../../stores/modStore";
	import { backgroundEnabled } from "../../stores/modStore";
	import {
		installationStatus,
		showWarningPopup,
	} from "../../stores/modStore";
	import { invoke } from "@tauri-apps/api/core";
	import { addMessage } from "$lib/stores";
	import UninstallDialog from "../../components/UninstallDialog.svelte";
	import { onMount } from "svelte";

	let currentSection = "mods";
	// window.addEventListener("resize", () => {
	//     console.log(
	//         `Window size: ${window.innerWidth} x ${window.innerHeight}`,
	//     );
	// });

	$: if (currentSection !== "mods") {
		// Store will retain the value but component won't show
		// Will reappear when returning to mods section
	}

	// Add these for the RequiresPopup
	let showRequiresPopup = false;

	let showUninstallDialog = false;
	let selectedMod = { name: "", path: "" };
	let dependents: string[] = [];

	async function handleRefresh() {
		const installedMods: InstalledMod[] = await invoke(
			"get_installed_mods_from_db",
		);
		installationStatus.set(
			Object.fromEntries(
				installedMods.map((mod: InstalledMod) => [mod.name, true]),
			),
		);
	}

	function showError(error: string) {
		addMessage(`Uninstall failed: ${error}`, "error");
	}

	let modRequirements = {
		steamodded: false,
		talisman: false,
	};

	function handleDependencyCheck(requirements: DependencyCheck) {
		modRequirements = requirements;
		showRequiresPopup = true;
	}

	onMount(() => {
		handleRefresh();
	});
</script>

{#if $backgroundEnabled}
	<ShaderBackground />
{/if}

<div class="main-page">
	<header>
		<div class="header-content">
			<h1>Balatro Mod Manager</h1>
			<LaunchButton />
		</div>
		<nav>
			<button
				class:active={currentSection === "mods"}
				on:click={() => (currentSection = "mods")}
			>
				Mods
			</button>
			<button
				class:active={currentSection === "settings"}
				on:click={() => (currentSection = "settings")}
			>
				Settings
			</button>
			<button
				class:active={currentSection === "about"}
				on:click={() => (currentSection = "about")}
			>
				About
			</button>
		</nav>
	</header>

	<div class="content">
		{#if currentSection === "mods"}
			<Mods
				{handleDependencyCheck}
				on:request_uninstall={(e) => {
					selectedMod = e.detail.mod;
					dependents = e.detail.dependents;
					showUninstallDialog = true;
				}}
			/>
		{/if}

		{#if currentSection === "settings"}
			<Settings />
		{/if}

		{#if currentSection === "about"}
			<About />
		{/if}
	</div>
	<RequiresPopup
		bind:show={showRequiresPopup}
		requiresSteamodded={modRequirements.steamodded}
		requiresTalisman={modRequirements.talisman}
	/>

	<WarningPopup
		visible={$showWarningPopup.visible}
		message={$showWarningPopup.message}
		onConfirm={$showWarningPopup.onConfirm}
		onCancel={$showWarningPopup.onCancel}
	/>
	<UninstallDialog
		bind:show={showUninstallDialog}
		modName={selectedMod.name}
		modPath={selectedMod.path}
		{dependents}
		on:uninstalled={handleRefresh}
		on:error={({ detail }) => showError(detail)}
	/>

	<div class="version-text">v0.1.3</div>
</div>

<style>
	.main-page {
		width: 100vw;
		height: 100vh;
		display: flex;
		flex-direction: column;
		padding: 2rem;
		box-sizing: border-box;
		background: transparent;
	}
	header {
		margin-bottom: -1rem;
	}

	h1 {
		color: #f4eee0;
		font-size: 3rem;
		margin-bottom: 2rem;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-2px -2px 0 #000,
			2px -2px 0 #000,
			-2px 2px 0 #000,
			2px 2px 0 #000;
	}

	nav {
		display: flex;
		gap: 1rem;
		margin-bottom: 2rem;
	}

	button {
		background: transparent;
		border: 2px solid #f4eee0;
		color: #f4eee0;
		padding: 0.7rem 1.4rem;
		border-radius: 8px;
		font-family: "M6X11", sans-serif;
		font-size: 1.2rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	button:hover,
	button.active {
		background: #f4eee0;
		color: #393646;
	}

	.content {
		flex: 1;
		background: rgba(193, 65, 57, 0.8);
		border-radius: 5px;
		backdrop-filter: blur(10px);
		padding: 2rem;
		margin-bottom: 2rem;
		outline: 2px solid #f4eee0;
		overflow-y: auto; /* Enable vertical scrolling */
		max-height: calc(100vh - 12rem);
		min-height: 0;

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

	.version-text {
		position: fixed;
		bottom: 1rem;
		right: 1rem;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000;
	}
	.header-content {
		position: relative;
		margin-bottom: 2rem;
	}
	header {
		margin-bottom: -1rem;
	}

	@media (max-width: 1160px) {
		button {
			padding: 0.6rem 1.2rem;
			border-radius: 8px;
			font-family: "M6X11", sans-serif;
			font-size: 0.9rem;
			cursor: pointer;
			transition: all 0.2s ease;
		}
	}
</style>
