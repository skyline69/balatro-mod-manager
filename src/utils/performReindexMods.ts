import { addMessage } from "$lib/stores";
import { invoke } from "@tauri-apps/api/core";

export async function performReindexMods() {
	try {
		await invoke("refresh_mods_folder");
		addMessage("Successfully re-indexed mods!", "success");
	} catch (error) {
		addMessage("Failed to re-index mods: " + error, "error");
	}
}

