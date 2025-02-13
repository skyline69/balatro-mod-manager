import { addMessage } from "$lib/stores";
import { invoke } from "@tauri-apps/api/core";
import { showWarningPopup } from "../stores/modStore";


export async function performReindexMods() {
	try {
		const hasUntracked = await invoke<boolean>('check_untracked_mods');

		if (hasUntracked) {
			showWarningPopup.set(true);
		} else {
			await invoke("refresh_mods_folder");
			addMessage("Mods re-indexed successfully", "success");
		}
	} catch (error) {
		addMessage("Failed to check mod status: " + error, "error");
	}
}

export async function confirmReindex() {
	try {
		await invoke("refresh_mods_folder");
		addMessage("Mods re-indexed successfully", "success");
	} catch (error) {
		addMessage("Failed to re-index mods: " + error, "error");
	}
	showWarningPopup.set(false);
}
