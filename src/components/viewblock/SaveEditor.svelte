<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { addMessage } from "$lib/stores";
	import { writable } from "svelte/store";
	import { onMount, onDestroy } from "svelte";
	import {
		Save,
		FileDown,
		RefreshCw,
		AlertTriangle,
		List,
		Folder,
		Edit,
	} from "lucide-svelte";

	// CodeMirror imports
	import { EditorState } from "@codemirror/state";
	import { EditorView, ViewUpdate, keymap } from "@codemirror/view";
	import { json } from "@codemirror/lang-json";
	import { indentUnit } from "@codemirror/language";
	import { oneDark } from "@codemirror/theme-one-dark";
	import { search, searchKeymap } from "@codemirror/search";

	// Create your own basic setup with necessary extensions including scrolling support
	const basicSetup = [
		EditorView.lineWrapping,
		EditorState.allowMultipleSelections.of(true),
		indentUnit.of("  "),
	];

	interface SaveDirectoryInfo {
		name: string;
		path: string;
		jkr_file_path: string | null;
		parsable: boolean;
		error_message: string | null;
	}

	let saveDirectories = $state<SaveDirectoryInfo[]>([]);
	let selectedDirectory = $state<SaveDirectoryInfo | null>(null);
	let loadedJkrPath = $state<string | null>(null);
	let saveData = $state<any>(null);
	let rawJson = $state<string>("");
	let isLoadingList = $state(false);
	let isLoadingFile = $state(false);
	let isSaving = $state(false);
	let isDirty = $state(false);
	let saveFolderPath = $state<string>("");

	const editorValid = writable(true);

	// CodeMirror setup
	let editorElement = $state<HTMLElement | null>(null);
	let editorView = $state<EditorView | null>(null);

	// Custom theme to override the oneDark theme and remove text stroke
	const customTheme = EditorView.theme({
		".cm-content": {
			fontFamily: "monospace",
			fontSize: "0.95rem",
			color: "#f4eee0",
			textShadow: "none",
		},
		".cm-line": {
			padding: "0 4px",
			textShadow: "none",
		},
		"&": {
			height: "100%",
			backgroundColor: "#2d2a3a",
		},
		".cm-scroller": {
			overflow: "auto",
			height: "100%",
		},
		// Make sure JSON syntax colors have no text stroke
		".cm-string": { textShadow: "none", color: "#a5d6a7" },
		".cm-number": { textShadow: "none", color: "#90caf9" },
		".cm-atom": { textShadow: "none", color: "#f48fb1" }, // true, false, null
		".cm-keyword": { textShadow: "none", color: "#f48fb1" },
		".cm-property": { textShadow: "none", color: "#ddbdf1" },
		".cm-operator": { textShadow: "none", color: "#ddbdf1" },
		".cm-selectionMatch": { backgroundColor: "rgba(253, 207, 81, 0.3)" },
		".cm-searchMatch": {
			backgroundColor: "rgba(253, 207, 81, 0.4)",
			outline: "1px solid #fdcf51",
		},
		".cm-searchMatch.cm-searchMatch-selected": {
			backgroundColor: "rgba(253, 207, 81, 0.7)",
		},
	});

	// Custom theme for the search panel
	const searchPanelTheme = EditorView.theme({
		".cm-panel": {
			backgroundColor: "#3a3648",
			color: "#f4eee0",
			border: "none",
			borderTop: "1px solid #4a4458",
		},
		".cm-panel input": {
			backgroundColor: "#2d2a3a",
			color: "#f4eee0",
			border: "1px solid #4a4458",
			borderRadius: "4px",
			padding: "4px 8px",
			outline: "none",
		},
		".cm-panel input:focus": {
			borderColor: "#fdcf51",
		},
		".cm-panel button": {
			backgroundColor: "#4f5a9c",
			color: "#f4eee0",
			border: "none",
			borderRadius: "4px",
			padding: "4px 8px",
			cursor: "pointer",
			marginLeft: "4px",
		},
		".cm-panel button:hover": {
			backgroundColor: "#606db7",
		},
		".cm-panel label": {
			color: "#f4eee0",
			margin: "0 8px",
		},
		".cm-panel-content": {
			padding: "8px",
		},
	});

	// Setup CodeMirror editor
	function setupEditor() {
		if (!editorElement) return;

		// Destroy previous instance if it exists
		if (editorView) {
			editorView.destroy();
		}

		// Create editor with appropriate extensions
		const extensions = [
			basicSetup,
			json(),
			indentUnit.of("  "),
			oneDark,
			customTheme, // Apply our custom theme after oneDark to override styles
			searchPanelTheme, // Add custom styling for the search panel
			search(), // Enable built-in search functionality
			keymap.of(searchKeymap), // Add the search key bindings (Ctrl+F, etc.)
			EditorView.updateListener.of((update: ViewUpdate) => {
				if (update.docChanged) {
					const value = update.state.doc.toString();
					if (rawJson !== value) {
						rawJson = value;
						handleJsonInput();
					}
				}
			}),
		];

		editorView = new EditorView({
			state: EditorState.create({
				doc: rawJson,
				extensions,
			}),
			parent: editorElement,
		});
	}

	// Update CodeMirror when rawJson changes externally
	$effect(() => {
		if (editorView && rawJson !== editorView.state.doc.toString()) {
			editorView.dispatch({
				changes: {
					from: 0,
					to: editorView.state.doc.length,
					insert: rawJson,
				},
			});
		}
	});

	// Set up editor when save data is loaded
	$effect(() => {
		if (saveData && editorElement && !isLoadingFile) {
			setTimeout(setupEditor, 0);
		}
	});

	async function listSaves() {
		isLoadingList = true;
		saveDirectories = [];
		selectedDirectory = null;
		loadedJkrPath = null;
		saveData = null;
		rawJson = "";
		isDirty = false;
		editorValid.set(true);

		try {
			try {
				saveFolderPath = await invoke("get_balatro_save_path");
			} catch (pathError) {
				console.warn("Could not get base save path:", pathError);
				saveFolderPath = "Unknown";
			}

			const dirs = await invoke<SaveDirectoryInfo[]>(
				"list_save_directories",
			);
			saveDirectories = dirs;
			if (dirs.length === 0 && saveFolderPath !== "Unknown") {
				addMessage(
					`No save profiles found in ${saveFolderPath}. Play Balatro to create saves.`,
					"info",
				);
			} else if (dirs.length === 0) {
				addMessage(
					`No save profiles found. Could not determine save path.`,
					"warning",
				);
			}
		} catch (error) {
			console.error("Failed to list save directories:", error);
			addMessage(`Error listing saves: ${error}`, "error");
		} finally {
			isLoadingList = false;
		}
	}

	async function loadSelectedSave(directory: SaveDirectoryInfo) {
		if (!directory.jkr_file_path) {
			addMessage(
				`Cannot load '${directory.name}': No primary .jkr file found inside.`,
				"warning",
			);
			return;
		}
		if (!directory.parsable) {
			addMessage(
				`Cannot load '${directory.name}': ${directory.error_message || "File is unparsable."}`,
				"error",
			);
			return;
		}
		if (isDirty) {
			const discard = confirm(
				"You have unsaved changes. Are you sure you want to discard them and load a new file?",
			);
			if (!discard) return;
		}

		isLoadingFile = true;
		isDirty = false;
		editorValid.set(true);
		selectedDirectory = directory;
		loadedJkrPath = directory.jkr_file_path;

		try {
			const data = await invoke("load_save_file", {
				path: loadedJkrPath,
			});
			saveData = data;
			rawJson = JSON.stringify(saveData, null, 2);
		} catch (error) {
			console.error("Failed to load save file:", error);
			addMessage(`Error loading save: ${error}`, "error");
			selectedDirectory = null;
			loadedJkrPath = null;
			saveData = null;
			rawJson = "";
		} finally {
			isLoadingFile = false;
		}
	}

	async function saveFile() {
		if (
			!loadedJkrPath ||
			!saveData ||
			!isDirty ||
			!$editorValid ||
			isSaving
		) {
			if (!isDirty) addMessage("No changes to save.", "info");
			if (!$editorValid) addMessage("Invalid JSON in editor.", "error");
			return;
		}

		try {
			isSaving = true;
			const updatedData = JSON.parse(rawJson); // Validate before saving
			await invoke("save_modified_file", {
				path: loadedJkrPath,
				data: updatedData,
			});
			saveData = updatedData; // Update internal state if needed
			isDirty = false;
			addMessage("Save file updated successfully!", "success");
		} catch (error) {
			console.error("Failed to save file:", error);
			addMessage(`Error saving file: ${error}`, "error");
		} finally {
			isSaving = false;
		}
	}

	function handleJsonInput() {
		isDirty = true;
		try {
			JSON.parse(rawJson); // Just validate
			editorValid.set(true);
		} catch (e) {
			editorValid.set(false);
		}
	}

	function handleDownload() {
		if (!saveData || !$editorValid) {
			if (!$editorValid) addMessage("Invalid JSON in editor.", "error");
			return;
		}
		try {
			const updatedData = JSON.parse(rawJson);
			const blob = new Blob([JSON.stringify(updatedData, null, 2)], {
				type: "application/json",
			});
			const url = URL.createObjectURL(blob);
			const a = document.createElement("a");
			a.href = url;
			const jkrFileName = loadedJkrPath?.split(/[\\/]/).pop() ?? "data";
			const downloadName = selectedDirectory
				? `${selectedDirectory.name}_${jkrFileName}.json`
				: "save_data.json";
			a.download = downloadName;
			a.click();
			URL.revokeObjectURL(url);
			addMessage("JSON data downloaded.", "success");
		} catch (e) {
			addMessage(`Failed to download JSON: ${e}`, "error");
		}
	}

	function goBackToList() {
		if (isDirty) {
			const discard = confirm(
				"You have unsaved changes. Are you sure you want to discard them and go back to the list?",
			);
			if (!discard) return;
		}

		// Clean up editor
		if (editorView) {
			editorView.destroy();
			editorView = null;
		}

		selectedDirectory = null;
		loadedJkrPath = null;
		saveData = null;
		rawJson = "";
		isDirty = false;
		editorValid.set(true);
	}

	async function openSaveFolder() {
		try {
			if (saveFolderPath && saveFolderPath !== "Unknown") {
				await invoke("open_directory", { path: saveFolderPath });
			} else {
				addMessage(
					"Could not determine the save folder path.",
					"error",
				);
			}
		} catch (error) {
			addMessage(`Failed to open save directory: ${error}`, "error");
		}
	}

	onMount(() => {
		listSaves();
	});

	onDestroy(() => {
		if (editorView) {
			editorView.destroy();
		}
	});
</script>

<div class="container default-scrollbar">
	<div class="save-editor-container">
		<h2>Save Editor</h2>

		{#if !selectedDirectory}
			<!-- Directory Listing View -->
			<div class="controls">
				<button onclick={listSaves} disabled={isLoadingList}>
					<RefreshCw size={18} />
					{isLoadingList ? "Scanning..." : "Refresh List"}
				</button>
				{#if saveFolderPath && saveFolderPath !== "Unknown"}
					<button
						onclick={openSaveFolder}
						title="Open the main Balatro save folder"
					>
						<Folder size={18} /> Open Save Folder
					</button>
					<span class="save-path-display" title={saveFolderPath}
						>Save Path: ...{saveFolderPath.slice(-40)}</span
					>
				{/if}
			</div>

			{#if isLoadingList}
				<p class="loading-placeholder">Scanning for save profiles...</p>
			{:else if saveDirectories.length > 0}
				<p class="list-header">
					Select a profile directory to load its save file:
				</p>
				<div class="directory-list default-scrollbar">
					{#each saveDirectories as dir (dir.path)}
						<button
							class="directory-item"
							class:unparsable={!dir.parsable}
							onclick={() => loadSelectedSave(dir)}
							title={!dir.jkr_file_path
								? `Cannot load '${dir.name}': No primary .jkr file found.`
								: !dir.parsable
									? `Cannot load '${dir.name}': ${dir.error_message ?? "File unparsable"}`
									: `Load save from '${dir.name}' (${dir.jkr_file_path?.split(/[\\/]/).pop() ?? "?.jkr"})`}
							disabled={isLoadingFile ||
								!dir.jkr_file_path ||
								!dir.parsable}
						>
							<span class="dir-name">{dir.name}</span>
							<span class="jkr-name"
								>{dir.jkr_file_path?.split(/[\\/]/).pop() ??
									"No .jkr"}</span
							>
							{#if !dir.parsable && dir.jkr_file_path}
								<span
									class="status-icon error"
									title={dir.error_message ?? "Unparsable"}
								>
									<AlertTriangle size={16} />
								</span>
							{:else if !dir.jkr_file_path}
								<span
									class="status-icon warning"
									title="No primary .jkr file found"
								>
									<AlertTriangle size={16} />
								</span>
							{/if}
						</button>
					{/each}
				</div>
			{:else}
				<p class="placeholder-text">
					No Balatro save profiles containing .jkr files found.
					<br />
					Play Balatro to generate saves, then refresh.
				</p>
			{/if}
		{:else}
			<!-- Editor View -->
			<div class="controls">
				<button
					onclick={goBackToList}
					disabled={isSaving || isLoadingFile}
				>
					<List size={18} />
					Back to List
				</button>
				<button
					onclick={saveFile}
					disabled={isSaving ||
						isLoadingFile ||
						!isDirty ||
						!$editorValid}
					class:invalid={!$editorValid && isDirty}
					title={$editorValid
						? `Save changes back to ${loadedJkrPath?.split(/[\\/]/).pop() ?? "file"}`
						: "Cannot save: Invalid JSON"}
				>
					<Save size={18} />
					{isSaving ? "Saving..." : "Save Changes"}
				</button>
				<button
					onclick={handleDownload}
					disabled={isSaving || isLoadingFile || !$editorValid}
					class:invalid={!$editorValid}
					title={$editorValid
						? "Download the current editor content as a JSON file"
						: "Cannot download: Invalid JSON"}
				>
					<FileDown size={18} />
					Download as JSON
				</button>
			</div>

			<p class="file-info">
				<Edit
					size={14}
					style="margin-right: 4px; vertical-align: middle;"
				/>
				Editing Profile: <strong>{selectedDirectory.name}</strong>
				({loadedJkrPath?.split(/[\\/]/).pop() ?? "N/A"})
				{#if isDirty}
					<span class="unsaved-indicator">*</span>
				{/if}
			</p>

			{#if isLoadingFile}
				<p class="loading-placeholder">Loading save data...</p>
			{:else if saveData}
				<div class="editor-area">
					<div
						bind:this={editorElement}
						class="code-editor-container"
						class:invalid={!$editorValid}
					></div>

					{#if !$editorValid}
						<p class="error-message">Invalid JSON format!</p>
					{/if}
				</div>
			{/if}
		{/if}
	</div>
</div>

<style>
	/* Main container structure - matches Settings.svelte */
	.container {
		width: 100%;
		height: 100%;
		overflow-y: auto;
	}

	.save-editor-container {
		padding: 0rem 2rem;
		padding-bottom: 2rem;

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

	/* CodeMirror editor container */
	.code-editor-container {
		width: 100%;
		height: 100%;
		flex-grow: 1;
		border-radius: 6px;
		border: 2px solid #4a4458;
		overflow: hidden;
		cursor: text;

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

	.code-editor-container.invalid {
		border-color: #f87171;
	}

	/* Ensure editor area has appropriate height and scrolling */
	.editor-area {
		flex-grow: 1;
		display: flex;
		flex-direction: column;
		min-height: 300px;
		height: 65vh; /* Changed from max-height to height for better size control */
		position: relative;
		overflow: hidden;
	}

	h2 {
		font-size: 2.5rem;
		margin-bottom: 2rem;
		color: #fdcf51;
	}

	.controls {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		margin-bottom: 1rem;
		flex-shrink: 0;
	}

	.controls button {
		background: #4f5a9c;
		outline: #3a4275 solid 2px;
		color: #f4eee0;
		border: none;
		border-radius: 6px;
		padding: 0.6rem 1.2rem;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		white-space: nowrap;
	}

	.controls button:hover:not(:disabled) {
		background: #606db7;
		transform: translateY(-2px);
	}

	.controls button:disabled {
		opacity: 0.7;
		cursor: not-allowed;
		transform: none;
	}

	.controls button.invalid {
		background-color: #b91c1c;
		outline-color: #991b1b;
	}
	.controls button.invalid:hover {
		background-color: #dc2626;
	}

	.save-path-display {
		font-size: 0.9rem;
		color: #b0adc8;
		align-self: center;
		margin-left: auto;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 300px;
		flex-shrink: 1;
	}

	.list-header {
		font-size: 1.1rem;
		color: #c4c2c2;
		margin-bottom: 0.75rem;
		flex-shrink: 0;
	}

	.directory-list {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin-top: 0.5rem;
		flex-grow: 1;
		overflow-y: auto;
		min-height: 150px;
		padding-bottom: 1rem;
		padding-right: 5px;
		max-height: 65vh;
	}

	.directory-item {
		background: #4a4458;
		color: #f4eee0;
		border: 1px solid #6a6478;
		border-radius: 4px;
		padding: 0.75rem 1rem;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		cursor: pointer;
		text-align: left;
		transition: all 0.2s ease;
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		box-sizing: border-box;
	}

	.directory-item:hover:not(:disabled) {
		background: #605a70;
		border-color: #8a8498;
		transform: translateX(3px);
	}

	.directory-item:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.directory-item.unparsable:not(:disabled) {
		opacity: 0.8;
	}
	.directory-item.unparsable:hover:not(:disabled) {
		background: #7a3b3b;
		border-color: #f87171;
	}

	.dir-name {
		flex-grow: 1;
		margin-right: 1rem;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 60%;
	}
	.jkr-name {
		color: #b0adc8;
		font-size: 0.9em;
		margin-right: auto;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 30%;
	}

	.status-icon {
		display: flex;
		align-items: center;
		flex-shrink: 0;
		margin-left: 0.5rem;
	}
	.status-icon.error {
		color: #f87171;
	}
	.status-icon.warning {
		color: #facc15;
	}

	.file-info {
		margin-bottom: 1rem;
		font-size: 1.1rem;
		color: #f4eee0;
		flex-shrink: 0;
		display: flex;
		align-items: center;
	}
	.file-info strong {
		color: #fdcf51;
	}
	.unsaved-indicator {
		color: #f87171;
		font-weight: bold;
		margin-left: 0.25rem;
	}

	.placeholder-text,
	.loading-placeholder {
		text-align: center;
		color: #a09da8;
		font-size: 1.2rem;
		margin-top: 3rem;
		line-height: 1.5;
		flex-grow: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.error-message {
		color: #f87171;
		font-size: 0.9rem;
		margin-top: 0.5rem;
		flex-shrink: 0;
		padding-bottom: 5px;
		height: 1.2em;
	}

	/* Global scrollbar styling matching Settings.svelte */
	.default-scrollbar::-webkit-scrollbar {
		width: 10px;
	}

	.default-scrollbar::-webkit-scrollbar-track {
		background: transparent;
		border-radius: 15px;
	}

	.default-scrollbar::-webkit-scrollbar-thumb {
		background: #f4eee0;
		border: 2px solid rgba(193, 65, 57, 0.8);
		border-radius: 15px;
	}

	@media (max-width: 1160px) {
		.save-editor-container {
			padding: 0rem 1rem;
			padding-bottom: 2rem;
		}
		h2 {
			font-size: 2rem;
		}
		.controls button {
			font-size: 1rem;
			padding: 0.5rem 1rem;
		}
		.directory-item {
			font-size: 1rem;
			padding: 0.6rem 0.8rem;
		}
		.file-info {
			font-size: 1rem;
		}
		.save-path-display {
			font-size: 0.8rem;
			max-width: 150px;
		}
		.list-header {
			font-size: 1rem;
		}
	}
</style>
