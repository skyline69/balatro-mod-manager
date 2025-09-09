import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";

export interface SimpleModDir {
  name: string; // folder display name
  path: string; // absolute path
  dir_name: string; // folder name only
  active: boolean; // whether currently under Mods
}

export interface ModProfile {
  id: number;
  name: string;
  mods: string[]; // absolute paths
}

export const profilesStore = writable<ModProfile[]>([]);
export const activeProfileId = writable<number | null>(null);
export const installedDirsStore = writable<SimpleModDir[]>([]);

export async function refreshInstalledDirs(): Promise<void> {
  const dirs = await invoke<SimpleModDir[]>("list_installed_mod_dirs");
  installedDirsStore.set(dirs);
}

export async function refreshProfiles(): Promise<void> {
  const profs = await invoke<ModProfile[]>("list_profiles");
  profilesStore.set(profs);
  const active = await invoke<number | null>("get_active_profile_id");
  activeProfileId.set(active);
}

export async function createProfile(name: string): Promise<number> {
  const id = await invoke<number>("create_profile", { name });
  await refreshProfiles();
  return id;
}

export async function deleteProfile(id: number): Promise<void> {
  await invoke("delete_profile", { id });
  await refreshProfiles();
}

export async function renameProfile(
  id: number,
  newName: string,
): Promise<void> {
  await invoke("rename_profile", { id, newName });
  await refreshProfiles();
}

export async function saveProfileMods(
  id: number,
  modDirNames: string[],
): Promise<void> {
  await invoke("set_profile_mods", { id, modDirNames });
  await refreshProfiles();
}

export async function activateProfile(id: number): Promise<void> {
  await invoke("apply_profile", { id });
  const active = await invoke<number | null>("get_active_profile_id");
  activeProfileId.set(active);
}

export async function deactivateProfiles(): Promise<void> {
  await invoke("clear_active_profile");
  activeProfileId.set(null);
}
