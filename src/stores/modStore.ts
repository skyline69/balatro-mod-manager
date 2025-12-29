import { writable, type Writable } from "svelte/store";

export interface Mod {
  title: string;
  description: string;
  image: string;
  imageFallback?: string;
  // Internal optional fields used by views/cache
  _dirName?: string;
  _installedPath?: string;
  categories: Category[];
  colors: {
    color1: string;
    color2: string;
  };
  requires_steamodded: boolean;
  requires_talisman: boolean;
  publisher: string;
  repo: string;
  downloadURL: string;
  folderName?: string | null;
  version?: string | null;
  installed: boolean;
  last_updated: number;
}

export interface LocalMod {
  name: string;
  id: string;
  author: string[];
  description: string;
  prefix: string;
  version?: string;
  path: string;
  dependencies: string[];
  conflicts: string[];
  is_tracked: boolean;
}

export enum SortOption {
  NameAsc = "name_asc",
  NameDesc = "name_desc",
  LastUpdatedAsc = "updated_asc",
  LastUpdatedDesc = "updated_desc",
  DownloadsAsc = "downloads_asc",
  DownloadsDesc = "downloads_desc",
}

export const backgroundEnabled = writable(false);

export const currentSort = writable<SortOption>(SortOption.LastUpdatedDesc);

export const updateAvailableStore = writable<{ [key: string]: boolean }>({});

export const modEnabledStore = writable<Record<string, boolean>>({});

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
  dependents: [],
});

export const selectedModStore = writable<{ name: string; path: string } | null>(
  null,
);
export const dependentsStore = writable<string[]>([]);
export const currentPage = writable(1);
export const itemsPerPage = writable(12);

export type UninstallResult = {
  success: boolean;
  action: "cascade" | "force" | "single";
};

export const cachedVersions = writable<{
  steamodded: string[];
  talisman: string[];
}>({
  steamodded:
    typeof window !== "undefined"
      ? JSON.parse(localStorage.getItem("version-cache-steamodded") || "[]")
      : [],
  talisman:
    typeof window !== "undefined"
      ? JSON.parse(localStorage.getItem("version-cache-talisman") || "[]")
      : [],
});

if (typeof window !== "undefined") {
  cachedVersions.subscribe((value) => {
    try {
      localStorage.setItem(
        "version-cache-steamodded",
        JSON.stringify(value.steamodded),
      );
      localStorage.setItem(
        "version-cache-talisman",
        JSON.stringify(value.talisman),
      );
    } catch (_) {
      // Ignore storage quota errors; caching is optional.
    }
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
  API = 6,
}

export const currentModView = writable<Mod | null>(null);
export const currentJokerView = writable<Mod | null>(null);
export const searchResults = writable<Mod[]>([]);
export const modsStore = writable<Mod[]>([]);

let persistSuspended = false;
let persistPending = false;
export async function withModsCachePersistenceSuspended<T>(
  fn: () => Promise<T>,
) {
  persistSuspended = true;
  try {
    return await fn();
  } finally {
    persistSuspended = false;
    if (persistPending) {
      persistPending = false;
      try {
        const current = await new Promise<Mod[]>((resolve) =>
          modsStore.subscribe((v) => {
            resolve(v);
          })(),
        );
        persistModsCache(current);
      } catch (_) {
        // ignore
      }
    }
  }
}

function persistModsCache(value: Mod[]) {
  try {
    const slim = value.map((m) => ({
      title: m.title,
      categories: m.categories,
      colors: m.colors,
      requires_steamodded: m.requires_steamodded,
      requires_talisman: m.requires_talisman,
      publisher: m.publisher,
      repo: m.repo,
      downloadURL: m.downloadURL,
      folderName: m.folderName ?? null,
      version: m.version ?? null,
      installed: m.installed,
      last_updated: m.last_updated,
      _dirName: m._dirName,
      _installedPath: m._installedPath,
      // omit description and image fields (largest strings)
    }));

    localStorage.setItem("mods-cache", JSON.stringify(slim));
    const now = Date.now();
    localStorage.setItem("mods-cache-ts", String(now));
    catalogLastRefreshed.set(now);
  } catch (e) {
    try {
      localStorage.removeItem("mods-cache");
      localStorage.removeItem("mods-cache-ts");
    } catch (_) {
      // ignore
    }
  }
}

// Background catalog loading state and last refresh time
export const catalogLoading = writable(false);
export const catalogLastRefreshed = writable<number | null>(null);

// Persist and hydrate the mods catalog for instant UI + offline fallback
if (typeof window !== "undefined") {
  try {
    const cached = localStorage.getItem("mods-cache");
    if (cached) {
      const parsed: Mod[] = JSON.parse(cached);
      if (Array.isArray(parsed)) {
        modsStore.set(parsed);
      }
    }
    const ts = localStorage.getItem("mods-cache-ts");
    if (ts) {
      const n = Number(ts);
      if (!Number.isNaN(n)) catalogLastRefreshed.set(n);
    }
  } catch (_) {
    // ignore cache read errors
  }

  let persistTimer: number | null = null;
  modsStore.subscribe((value) => {
    if (persistTimer) {
      clearTimeout(persistTimer);
    }
    // Debounce cache writes to avoid thrashing localStorage during hydration.
    persistTimer = window.setTimeout(() => {
      if (persistSuspended) {
        persistPending = true;
        return;
      }
      persistModsCache(value);
    }, 250);
  });
}

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
  const storedCategory = localStorage.getItem("currentCategory") || "Popular";
  const { subscribe, set } = writable(storedCategory);

  return {
    subscribe,
    set: (value: string) => {
      try {
        localStorage.setItem("currentCategory", value);
      } catch (_) {
        // Ignore storage quota errors.
      }
      set(value);
    },
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
  onConfirm: () => {},
  onCancel: () => {},
});

// Popup state to warn users when Lovely is not detected
export interface LovelyPopupState {
  visible: boolean;
  source?: "launch" | "other";
  onLaunchAnyway?: () => void | Promise<void>;
}

export const lovelyPopupStore = writable<LovelyPopupState>({
  visible: false,
});
