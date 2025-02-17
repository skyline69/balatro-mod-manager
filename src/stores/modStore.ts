import { writable, type Writable } from 'svelte/store';

export interface Mod {
	title: string;
	description: string;
	image: string;
	lastUpdated: string;
	categories: Category[];
	colors: { color1: string; color2: string };
	installed: boolean;
	requires_steamodded: boolean;
	requires_talisman: boolean;
	publisher: string;
	repo: string;
	downloadURL: string;
}

export enum SortOption {
	NameAsc = "name_asc",
	NameDesc = "name_desc",
	LastUpdatedAsc = "updated_asc",
	LastUpdatedDesc = "updated_desc"
}

export const backgroundEnabled = writable(false);

export const currentSort = writable<SortOption>(SortOption.NameAsc);

export interface UninstallDialogState {
	show: boolean;
	modName: string;
	modPath: string;
	dependents: string[];
}

export const uninstallDialogStore = writable<UninstallDialogState>({
	show: false,
	modName: "",
	modPath: "",
	dependents: []
});

export const selectedModStore = writable<{ name: string; path: string } | null>(null);
export const dependentsStore = writable<string[]>([]);

export type UninstallResult = {
	success: boolean;
	action: "cascade" | "force" | "single";
};

export const cachedVersions = writable<{
	steamodded: string[];
	talisman: string[];
}>({
	steamodded: typeof window !== 'undefined'
		? JSON.parse(localStorage.getItem('version-cache-steamodded') || '[]')
		: [],
	talisman: typeof window !== 'undefined'
		? JSON.parse(localStorage.getItem('version-cache-talisman') || '[]')
		: []
});

if (typeof window !== 'undefined') {
	cachedVersions.subscribe(value => {
		localStorage.setItem('version-cache-steamodded', JSON.stringify(value.steamodded));
		localStorage.setItem('version-cache-talisman', JSON.stringify(value.talisman));
	});
}

export interface DependencyCheck {
	steamodded: boolean;
	talisman: boolean;
}

export interface InstalledMod {
	name: string;
	path: string;
	// collection_hash: string | null;
}

interface InstallationStatus {
	[key: string]: boolean;
}


export enum Category {
	Content = 0,
	Joker = 1,
	QualityOfLife = 2,
	Technical = 3,
	Miscellaneous = 4,
	ResourcePacks = 5,
	API = 6
}


export const currentModView = writable<Mod | null>(null);
export const currentJokerView = writable<Mod | null>(null);
export const searchResults = writable<Mod[]>([]);
export const modsStore = writable<Mod[]>([]);


export const installationStatus: Writable<InstallationStatus> = writable({});

export const loadingStates2 = writable<{ [key: string]: boolean }>({});
//
//
// modsStore.subscribe(value => {
// 	if (typeof window !== 'undefined') {
// 		localStorage.setItem('mods', JSON.stringify(value));
// 	}
// });


function createPersistentCategory() {
	const storedCategory = localStorage.getItem('currentCategory') || "Popular";
	const { subscribe, set } = writable(storedCategory);

	return {
		subscribe,
		set: (value: string) => {
			localStorage.setItem('currentCategory', value);
			set(value);
		}
	};
}

export const currentCategory = createPersistentCategory();


export interface WarningPopupState {
	visible: boolean;
	message: string;
	onConfirm: () => void;
	onCancel: () => void;
}

export const showWarningPopup = writable<WarningPopupState>({
	visible: false,
	message: "",
	onConfirm: () => { },
	onCancel: () => { }
});

