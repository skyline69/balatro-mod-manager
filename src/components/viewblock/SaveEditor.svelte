<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { addMessage } from "$lib/stores";
	import { writable } from "svelte/store";
	import { onMount, onDestroy } from "svelte";
	import {
		Save,
		RefreshCw,
		AlertTriangle,
		List,
		Folder,
		Edit,
		X,
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

	// Modal state
	let showConfirmModal = $state(false);
	let confirmAction = $state<(() => void) | null>(null);
	let confirmMessage = $state("");
	let confirmTitle = $state("");
	let confirmButtonRef = $state<HTMLButtonElement | null>(null);
	let modalElement = $state<HTMLElement | null>(null);
	let lastActiveElement: HTMLElement | null = null; // Changed type to HTMLElement
	let editedSavePaths = $state<Set<string>>(new Set());
	let confirmButtonLabel = $state("Discard Changes"); // Default button label
	let proceedAnywayPaths = $state<Set<string>>(new Set());
	let editedSaveTimestamps = $state<Map<string, number>>(new Map()); // Track when we last edited each file

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
			// Add custom scrollbar styling
			"&::-webkit-scrollbar": {
				width: "10px",
			},
			"&::-webkit-scrollbar-track": {
				background: "transparent",
				borderRadius: "15px",
			},
			"&::-webkit-scrollbar-thumb": {
				background: "#f4eee0",
				border: "2px solid rgba(193, 65, 57, 0.8)",
				borderRadius: "15px",
			},
			"&::-webkit-scrollbar:horizontal": {
				display: "none",
			},
			"&::-webkit-scrollbar-corner": {
				backgroundColor: "transparent",
			},
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

	// Set focus to the confirm button when modal opens
	$effect(() => {
		if (showConfirmModal && confirmButtonRef) {
			setTimeout(() => {
				// Check again in case it changed during the timeout
				if (confirmButtonRef) {
					confirmButtonRef.focus();
				}
			}, 50);
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

		// Don't reset editedSavePaths here, as we want to remember which saves were edited
		// But we can reset the proceedAnywayPaths as a refresh could indicate restart of Balatro
		proceedAnywayPaths = new Set();

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
					`No saves found in ${saveFolderPath}. Play Balatro to create saves.`,
					"info",
				);
			} else if (dirs.length === 0) {
				addMessage(
					`No saves found. Could not determine save path.`,
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

	function showConfirmDialog(
		title: string,
		message: string,
		onConfirm: () => void,
		buttonLabel: string = "Discard Changes",
	) {
		// Store the currently focused element to restore focus when closing
		const activeElement = document.activeElement;
		if (activeElement instanceof HTMLElement) {
			lastActiveElement = activeElement;
		}

		confirmTitle = title;
		confirmMessage = message;
		confirmAction = onConfirm;
		confirmButtonLabel = buttonLabel; // Set the button label
		showConfirmModal = true;
	}

	function closeConfirmDialog() {
		showConfirmModal = false;
		confirmAction = null;

		// Restore focus to the element that was focused before the modal opened
		if (lastActiveElement) {
			setTimeout(() => {
				if (lastActiveElement) {
					lastActiveElement.focus();
				}
			}, 10);
		}
	}

	function handleConfirm() {
		if (confirmAction) {
			const action = confirmAction;
			closeConfirmDialog();
			action();
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (!showConfirmModal) return;

		if (event.key === "Escape") {
			event.preventDefault();
			closeConfirmDialog();
		} else if (
			event.key === "Enter" &&
			document.activeElement === confirmButtonRef
		) {
			event.preventDefault();
			handleConfirm();
		}
	}

	async function hasFileBeenModifiedSinceEdit(
		filePath: string,
	): Promise<boolean> {
		try {
			if (!editedSaveTimestamps.has(filePath)) {
				return false; // We haven't edited it, so no need to check
			}

			// Get the current file modification time
			const currentModTime = await invoke<number>(
				"get_file_last_modified",
				{
					path: filePath,
				},
			);

			// Compare with our stored edit time
			const ourEditTime = editedSaveTimestamps.get(filePath) || 0;

			// If the file's modification time is newer than our edit time, Balatro likely ran
			return currentModTime > ourEditTime;
		} catch (error) {
			console.error("Error checking file modification time:", error);
			return false; // Assume no modification on error
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

		// Check if this save was previously edited but user hasn't chosen to proceed with it yet
		if (directory.jkr_file_path) {
			// Check if this save has been edited before
			if (editedSavePaths.has(directory.jkr_file_path)) {
				// If user already clicked "Proceed Anyway" for this save, skip the warning
				if (proceedAnywayPaths.has(directory.jkr_file_path)) {
					loadSaveImplementation(directory);
					return;
				}

				// Check if the file has been modified since our last edit (Balatro ran)
				const fileModified = await hasFileBeenModifiedSinceEdit(
					directory.jkr_file_path,
				);

				if (fileModified) {
					// Balatro ran and modified the file, so remove it from our edited list
					editedSavePaths.delete(directory.jkr_file_path);
					editedSaveTimestamps.delete(directory.jkr_file_path);
					loadSaveImplementation(directory);
					return;
				}

				// File hasn't been modified by Balatro, show warning
				showConfirmDialog(
					"Run Balatro First",
					"You have previously edited this save file. To safely edit it again, you should run Balatro first to let it process your changes. Do you want to proceed anyway?",
					() => {
						// User confirmed they want to proceed anyway
						proceedAnywayPaths.add(directory.jkr_file_path!);
						loadSaveImplementation(directory);
					},
					"Proceed Anyway",
				);
				return;
			}
		}

		// Handle unsaved changes case
		if (isDirty) {
			showConfirmDialog(
				"Unsaved Changes",
				"You have unsaved changes. Are you sure you want to discard them and load a new file?",
				() => {
					// This will run when user confirms
					loadSaveImplementation(directory);
				},
			);
			return;
		}

		// If no warnings needed, load the save directly
		loadSaveImplementation(directory);
	}

	// Helper function to avoid code duplication
	async function loadSaveImplementation(directory: SaveDirectoryInfo) {
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

			// Track this save as edited
			if (loadedJkrPath) {
				editedSavePaths.add(loadedJkrPath);
				editedSaveTimestamps.set(loadedJkrPath, Date.now());
			}

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

	function goBackToList() {
		if (isDirty) {
			showConfirmDialog(
				"Unsaved Changes",
				"You have unsaved changes. Are you sure you want to discard them and go back to the list?",
				() => {
					// This will run when user confirms
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
				},
			);
			return;
		}

		// Only reset state if not dirty
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
		// Add global event listener for escape key
		window.addEventListener("keydown", handleKeyDown);
	});

	onDestroy(() => {
		if (editorView) {
			editorView.destroy();
		}
		window.removeEventListener("keydown", handleKeyDown);
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
				{/if}
			</div>

			{#if isLoadingList}
				<p class="loading-placeholder">Scanning for saves...</p>
			{:else if saveDirectories.length > 0}
				<p class="list-header">Select a save file:</p>
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
					No Balatro save containing .jkr files found.
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
			</div>

			<p class="file-info">
				<Edit
					size={14}
					style="margin-right: 4px; vertical-align: middle;"
				/>
				<span>Editing:</span>
				<span style="margin-left: 6px;"
					><strong>{selectedDirectory.name}</strong></span
				>
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

<!-- Confirmation Modal - Fixed for Accessibility -->
{#if showConfirmModal}
	<div
		bind:this={modalElement}
		class="modal-backdrop"
		role="dialog"
		aria-modal="true"
		aria-labelledby="modal-title"
		tabindex="-1"
	>
		<!-- The modal content -->
		<div class="modal-container">
			<div class="modal-header">
				<h3 id="modal-title">{confirmTitle}</h3>
				<button
					class="close-button"
					onclick={closeConfirmDialog}
					aria-label="Close dialog"
				>
					<X size={18} />
				</button>
			</div>
			<div class="modal-content">
				<p>{confirmMessage}</p>
			</div>
			<div class="modal-footer">
				<button class="cancel-button" onclick={closeConfirmDialog}
					>Cancel</button
				>
				<button
					class="confirm-button"
					onclick={handleConfirm}
					bind:this={confirmButtonRef}
				>
					{confirmButtonLabel}
				</button>
			</div>
		</div>

		<!-- Invisible button that covers the backdrop for accessibility -->
		<button
			class="backdrop-button"
			onclick={closeConfirmDialog}
			aria-label="Close dialog"
			tabindex="-1"
		>
		</button>
	</div>
{/if}

<style>
	.backdrop-button {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: transparent;
		border: none;
		z-index: -1; /* Below the modal container */
		cursor: default;
	}

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

	/* Modal Styles */
	.modal-backdrop {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background-color: rgba(0, 0, 0, 0.7);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		backdrop-filter: blur(2px);
	}

	.modal-container {
		background-color: #352e44;
		border-radius: 8px;
		width: 95%;
		max-width: 450px;
		box-shadow: 0 15px 30px rgba(0, 0, 0, 0.4);
		border: 2px solid #4a4458;
		animation: modal-appear 0.3s ease-out;
	}

	@keyframes modal-appear {
		from {
			opacity: 0;
			transform: translateY(-30px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.modal-header {
		padding: 1rem 1.5rem;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid #4a4458;
	}

	.modal-header h3 {
		margin: 0;
		color: #fdcf51;
		font-size: 1.5rem;
	}

	.close-button {
		background: transparent;
		border: none;
		color: #b0adc8;
		cursor: pointer;
		padding: 4px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 4px;
		transition: all 0.2s ease;
	}

	.close-button:hover {
		color: #f4eee0;
		background-color: rgba(255, 255, 255, 0.1);
	}

	.modal-content {
		padding: 1.5rem;
		color: #f4eee0;
		font-size: 1.1rem;
		line-height: 1.5;
	}

	.modal-footer {
		padding: 1rem 1.5rem;
		display: flex;
		justify-content: flex-end;
		gap: 1rem;
		border-top: 1px solid #4a4458;
	}

	.modal-footer button {
		padding: 0.6rem 1.2rem;
		border-radius: 6px;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.cancel-button {
		background-color: transparent;
		color: #f4eee0;
		border: 1px solid #6a6478;
	}

	.cancel-button:hover {
		background-color: rgba(255, 255, 255, 0.1);
		border-color: #8a8498;
	}

	.confirm-button {
		background-color: #b91c1c;
		color: #f4eee0;
		border: none;
		outline: #991b1b solid 2px;
	}

	.confirm-button:hover {
		background-color: #dc2626;
		transform: translateY(-2px);
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
		.list-header {
			font-size: 1rem;
		}
		.modal-header h3 {
			font-size: 1.3rem;
		}
		.modal-content {
			padding: 1.2rem;
			font-size: 1rem;
		}
		.modal-footer button {
			font-size: 1rem;
			padding: 0.5rem 1rem;
		}
	}
</style>
