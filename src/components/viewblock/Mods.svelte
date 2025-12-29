<script lang="ts">
	import {
		Download,
		Flame,
		//Clock,
		Star,
		Spade,
		Gamepad2,
		LayoutDashboard,
		FolderHeart,
		Search,
		BookOpen,
		Folder,
		RefreshCw,
	} from "lucide-svelte";
	import ModView from "./ModView.svelte";
	import { fly } from "svelte/transition";
	import {
		SortOption,
		backgroundEnabled,
		currentSort,
		loadingStates2,
	} from "../../stores/modStore";
	import { ArrowUpDown } from "lucide-svelte";
	import {
		currentModView,
		currentCategory,
		uninstallDialogStore,
	} from "../../stores/modStore";
	import type { LocalMod, Mod } from "../../stores/modStore";
	import { Category } from "../../stores/modStore";
import {
	modsStore,
	installationStatus,
	withModsCachePersistenceSuspended,
} from "../../stores/modStore";
import {
	setDescriptions,
	withDescriptionsPersistenceSuspended,
} from "../../stores/descriptions";
	import { catalogLoading } from "../../stores/modStore";
	import type { InstalledMod } from "../../stores/modStore";
	import { invoke, convertFileSrc } from "@tauri-apps/api/core";
// Lazy-load SearchView only when Search tab is active
import type { Component } from "svelte";
let SearchViewComp = $state<Component | null>(null);
import { onMount, onDestroy } from "svelte";
import { listen } from "@tauri-apps/api/event";
	import { writable } from "svelte/store";
	import { addMessage } from "$lib/stores";
	import { currentPage, itemsPerPage } from "../../stores/modStore";
	import ModCard from "./ModCard.svelte";
	import LocalModCard from "./LocalModCard.svelte";
import {
		checkModInCache,
		fetchCachedMods,
		forceRefreshCache,
	} from "../../stores/modCache";
	import { updateAvailableStore } from "../../stores/modStore";
	import { modEnabledStore } from "../../stores/modStore";
	import { browser } from "$app/environment";
import { openExternal } from "$lib/opener";

	const loadingDots = writable(0);
	let installedMods: InstalledMod[] = [];

	// Dedupe description loads across helpers
	const inflightDescriptions = new Set<string>();
	const attemptedDescriptions = new Set<string>();
	const attemptedCacheTitles = new Set<string>();
	let visibleFirstRunning = false;
	let visibleHydrateTimer: number | null = null;

	// Add these variables to track enabled/disabled mods
	let enabledMods: Mod[] = $state([]);
	let disabledMods: Mod[] = $state([]);
	let enabledLocalMods: LocalMod[] = $state([]);
	let disabledLocalMods: LocalMod[] = $state([]);

	// Animate the dots
	let dotInterval: number;
	let paginating = $state(false);
	let paginationIdleTimer: number | null = null;
	let hydrationTimer: number | null = null;
	let hydrationPending = false;
let isLinux = false;
let modsScrollContainer: HTMLDivElement | null = $state(null);
let scrollIdleTimer: number | null = null;
let isUserScrolling = $state(false);
const gridGap = 30;
let virtualPaddingTop = $state(0);
let virtualPaddingBottom = $state(0);
let virtualCols = $state(1);
let virtualRowHeight = $state(390);
const virtualOverscanRows = 2;
let renderLimitLocal = $state(60);
const renderChunkLocal = 24;
let localSentinel: HTMLDivElement | null = $state(null);
let modsGridEl: HTMLDivElement | null = $state(null);
let firstCardEl: HTMLElement | null = null;
let scrollRaf: number | null = null;
let resizeObs: ResizeObserver | null = null;

	async function handleModUninstalled() {
		// Refresh the local mods list
		getLocalMods();
		// Also refresh installed mods for consistency
		refreshInstalledMods();
	}

	// let mods: Mod[] = [];
	let isLoading = $state(true);

	interface DependencyCheck {
		steamodded: boolean;
		talisman: boolean;
	}

	let localMods: LocalMod[] = $state([]);
	let isLoadingLocalMods = $state(false);

	async function handleModToggled(): Promise<void> {
		if ($currentCategory === "Installed Mods") {
			// First check catalog mods
			for (const mod of paginatedMods) {
				if ($installationStatus[mod.title]) {
					try {
						const isEnabled = await invoke<boolean>(
							"is_mod_enabled",
							{
								modName: mod.title,
							},
						);
						modEnabledStore.update((s) => ({
							...s,
							[mod.title]: isEnabled,
						}));
					} catch (error) {
						console.error(
							`Failed to check catalog mod status: ${error}`,
						);
					}
				}
			}

			// Then check local mods via batch
			// (summary refresh handles local + catalog states)

			// Update filtered lists
			updateEnabledDisabledLists();

			// Force Svelte reactivity by creating new array references
			enabledMods = [...enabledMods];
			disabledMods = [...disabledMods];
			enabledLocalMods = [...enabledLocalMods];
			disabledLocalMods = [...disabledLocalMods];
		}
	}

	async function getLocalMods() {
		if ($currentCategory === "Installed Mods") {
			isLoadingLocalMods = true;
			try {
				localMods = await invoke("get_detected_local_mods");
				await refreshStateSummary();
			} catch (error) {
				console.error("Failed to load local mods:", error);
				addMessage(`Failed to load local mods: ${error}`, "error");
				localMods = [];
			} finally {
				isLoadingLocalMods = false;
			}
		}
	}

    // Avoid forcing a refresh on every reactive pass; only fetch local mods here.
    // We refresh installed mods on category switch and after install/uninstall events.
    $effect(() => {
        if ($currentCategory === "Installed Mods") {
            getLocalMods();
        }
    });

const { handleDependencyCheck, mod } = $props<{
    handleDependencyCheck: (
        requirements: DependencyCheck,
        downloadAction?: () => Promise<void>,
    ) => void;
    mod: Mod | null;
}>();

	// Update the pagination functions to reset scroll position when switching pages
	function nextPage() {
		if ($currentPage < totalPages) {
			currentPage.update((n) => n + 1);
			updatePaginationWindow();
			scrollToTop();
			markPaginating();
		}
	}

	function previousPage() {
		if ($currentPage > 1) {
			currentPage.update((n) => n - 1);
			updatePaginationWindow();
			scrollToTop();
			markPaginating();
		}
	}

	function goToPage(page: number) {
		currentPage.set(page);
		updatePaginationWindow();
		scrollToTop();
		markPaginating();
	}

	// Add this helper function to handle scrolling to top
	function scrollToTop() {
		const scrollContainer = document.querySelector(
			".mods-scroll-container",
		);
		if (scrollContainer) {
			scrollContainer.scrollTo({
				top: 0,
				behavior: isLinux ? "auto" : "smooth",
			});
		}
		setTimeout(() => {}, 500); // Delay to prevent scroll handler triggering during animated scroll
	}

	function markPaginating() {
		paginating = true;
		if (paginationIdleTimer) {
			clearTimeout(paginationIdleTimer);
		}
		paginationIdleTimer = window.setTimeout(() => {
			paginating = false;
			if (hydrationPending) scheduleHydration();
		}, isLinux ? 220 : 150);
	}

	function updateEnabledDisabledLists() {
		// Filter catalog mods - explicitly check for boolean values
		enabledMods = paginatedMods.filter(
			(mod) =>
				$installationStatus[mod.title] &&
				$modEnabledStore[mod.title] === true,
		);
		disabledMods = paginatedMods.filter(
			(mod) =>
				$installationStatus[mod.title] &&
				$modEnabledStore[mod.title] === false,
		);

		// Filter local mods - explicitly check for boolean values
		enabledLocalMods = localMods.filter(
			(mod) => $modEnabledStore[mod.name] === true,
		);
		disabledLocalMods = localMods.filter(
			(mod) => $modEnabledStore[mod.name] === false,
		);
	}

	// Update the lists whenever the stores change
	$effect(() => {
		if ($currentCategory === "Installed Mods") {
			updateEnabledDisabledLists();
		}
	});

	onMount(() => {
		// Animation dots initialization
		dotInterval = setInterval(() => {
			loadingDots.update((n) => (n + 1) % 4);
		}, 500);

		if (browser) {
			const plat =
				document.documentElement.dataset.platform ||
				(navigator.userAgent.toLowerCase().includes("linux")
					? "linux"
					: "");
			isLinux = plat === "linux";
		}

		// Separate async function for initialization
		const initialize = async () => {
			try {
				isLoading = true;
				// If the user is on Installed Mods, pre-seed placeholders so they are visible immediately
				if ($currentCategory === "Installed Mods") {
					await seedInstalledPlaceholders();
				}
				// If we have no cached catalog yet, do a foreground load for first-run
				if ($modsStore.length === 0) {
					await loadCatalogForeground();
				} else {
					// Otherwise, refresh in the background
				refreshCatalogInBackground();
				}

				// After mods load, update install status and local mods if needed
				try {
					const installed = await fetchCachedMods();
					const installedSet = new Set(
						installed.map((mod) => mod.name.toLowerCase()),
					);
					installationStatus.set(
						Object.fromEntries(
							$modsStore.map((mod) => [
								mod.title,
								installedSet.has(mod.title.toLowerCase()),
							]),
						),
					);
				} catch (error) {
					console.error("Install status check failed:", error);
				}

				// Fill local thumbnails for installed mods to avoid remote image fetches
				try {
					await fillInstalledThumbnails($modsStore);
				} catch (e) {
					console.warn("thumbnail fill failed", e);
				}

				if ($currentCategory === "Installed Mods") {
					await getLocalMods();
				}
			} finally {
				isLoading = false;
			}
		};

		// Separate async function for background state
		const initBackgroundState = async () => {
			try {
				const isBackgroundAnimationEnabled: boolean = await invoke(
					"get_background_state",
				);
				backgroundEnabled.set(isBackgroundAnimationEnabled);
			} catch (error) {
				console.error("Failed to get background status:", error);
				addMessage(
					"Error fetching background animation status",
					"error",
				);
			}
		};

		// Call async functions without awaiting them directly in onMount
		initialize();
		initBackgroundState();

		let installedModsRefresh: Promise<void> | null = null;
		const scheduleInstalledModsRefresh = () => {
			if (!installedModsRefresh) {
				installedModsRefresh = (async () => {
					await refreshInstalledMods();
					await getLocalMods();
					await refreshStateSummary();
				})().finally(() => {
					installedModsRefresh = null;
				});
			}
			return installedModsRefresh;
		};
		// Intersection observers to extend render window for local mods
		onMount(() => {
			if (typeof IntersectionObserver !== "undefined") {
				if (localSentinel) {
					observerLocal = new IntersectionObserver((entries) => {
						for (const entry of entries) {
							if (entry.isIntersecting) {
								const localMax = Math.max(
									enabledLocalMods.length,
									disabledLocalMods.length,
								);
								renderLimitLocal = Math.min(
									renderLimitLocal + renderChunkLocal,
									localMax,
								);
							}
						}
					}, { root: modsScrollContainer, rootMargin: "200px" });
					observerLocal.observe(localSentinel);
				}
			}
		});

		// Lazy-load SearchView when needed
			$effect(() => {
				if (showSearch && !SearchViewComp) {
					import("./SearchView.svelte")
						.then((m) => (SearchViewComp = m.default))
						.catch((err) =>
							console.warn("Failed to load SearchView:", err),
						);
				}
			});

			// Listen for backend notifications of installed mods changes
			let unlistenModsChanged: (() => void) | null = null;
			listen("installed-mods-changed", async () => {
				if ($currentCategory === "Installed Mods") {
					try {
						await scheduleInstalledModsRefresh();
					} catch (err) {
						console.warn("Failed to refresh installed mods:", err);
					}
				}
			})
				.then((un) => (unlistenModsChanged = un))
				.catch((err) =>
					console.warn("Failed to subscribe to installed-mods:", err),
				);

			// Return synchronous cleanup function
			return () => {
				clearInterval(dotInterval);
				if (scrollIdleTimer) {
					clearTimeout(scrollIdleTimer);
					scrollIdleTimer = null;
				}
				try {
					observerLocal?.disconnect();
				} catch (_) { /* ignore */ }
				try {
					if (typeof unlistenModsChanged === "function") unlistenModsChanged();
				} catch (err) {
					console.warn("Failed to unlisten installed-mods:", err);
				}
			};
		});

	const getAllInstalledMods = async () => {
		try {
			installedMods = await fetchCachedMods();
		} catch (error) {
			console.error("Failed to get installed mods:", error);
		}
	};

    const uninstallMod = async (mod: Mod) => {
        const isCoreMod = ["steamodded", "talisman"].includes(
            mod.title.toLowerCase(),
        );
        try {
            await getAllInstalledMods();
            const installedMod = installedMods.find(
                (m) => m.name.toLowerCase() === mod.title.toLowerCase(),
            );
            if (isCoreMod) {
                // Get dependents
                const dependents = await invoke<string[]>("get_dependents", {
                    modName: mod.title,
                });
                // Set the dialog properties directly
                uninstallDialogStore.set({
                    show: true,
                    modName: mod.title,
                    // Path may be resolved in the dialog if missing
                    modPath: installedMod?.path || "",
                    dependents,
                });
            } else {
                // For non-core mods
                if (!installedMod) return;
                await invoke("remove_installed_mod", {
                    name: mod.title,
                    path: installedMod.path,
                });
                installationStatus.update((s) => ({
                    ...s,
					[mod.title]: false,
				}));
			}
		} catch (error) {
			console.error("Uninstall failed:", error);
			addMessage(`Uninstall failed: ${error}`, "error");
		}
	};

    let hasUpdatesAvailable = $derived(
        Object.values($updateAvailableStore).some((value) => value === true)
    );

	async function updateAllMods(e?: Event) {
		if (e) e.preventDefault();

		try {
			// Get all installed mods with available updates
			const modsToUpdate = $modsStore.filter(
				(mod) =>
					$installationStatus[mod.title] &&
					$updateAvailableStore[mod.title],
			);

			if (modsToUpdate.length === 0) {
				addMessage("No updates available.", "info");
				return;
			}

			const previousEnabledMap: Record<string, boolean> = Object.fromEntries(
				await Promise.all(
					modsToUpdate.map(async (mod) => {
						const cached = $modEnabledStore?.[mod.title];
						if (cached !== undefined) {
							return [mod.title, cached] as const;
						}

						try {
							const enabled = await invoke<boolean>("is_mod_enabled", {
								modName: mod.title,
							});
							return [mod.title, enabled] as const;
						} catch (error) {
							console.error(
								`Failed to read enabled state for ${mod.title}:`,
								error,
							);
							return [mod.title, true] as const;
						}
					}),
				),
			);

			// Set loading state for all mods simultaneously
			for (const mod of modsToUpdate) {
				loadingStates2.update((s) => ({ ...s, [mod.title]: true }));
			}

			// Run all updates in parallel
			const updateResults = await Promise.allSettled(
				modsToUpdate.map(async (mod) => {
					try {
						if (mod.title.toLowerCase() === "steamodded") {
							const latestReleaseURL = await invoke<string>(
								"get_latest_steamodded_release",
							);
							await installModFromURL(mod, latestReleaseURL);
						} else if (mod.downloadURL) {
							const folderName =
								mod.folderName || mod.title.replace(/\s+/g, "");
							const installedPath = await invoke<string>(
								"install_mod",
								{
									url: mod.downloadURL,
									folderName,
								},
							);

							await invoke("add_installed_mod", {
								name: mod.title,
								path: installedPath,
								dependencies: mod.requires_steamodded
									? ["Steamodded"]
									: mod.requires_talisman
										? ["Talisman"]
										: [],
								currentVersion: mod.version || "",
							});
						} else {
							throw new Error("No download URL available");
						}

						// Update was successful
						return mod.title;
					} catch (error) {
						console.error(
							`Failed to update mod ${mod.title}:`,
							error,
						);
						throw new Error(
							`Failed to update ${mod.title}: ${error instanceof Error ? error.message : String(error)}`,
						);
					}
				}),
			);

			// Process results
			const successful: string[] = [];
			const failed: string[] = [];

			updateResults.forEach((result, index) => {
				const modTitle = modsToUpdate[index].title;

				// Clear loading state
				loadingStates2.update((s) => ({ ...s, [modTitle]: false }));

				if (result.status === "fulfilled") {
					successful.push(modTitle);
					// Mark as updated
					updateAvailableStore.update((s) => ({
						...s,
						[modTitle]: false,
					}));
					modEnabledStore.update((s) => ({
						...s,
						[modTitle]: previousEnabledMap[modTitle],
					}));
				} else {
					failed.push(modTitle);
					// Show error message
					addMessage(result.reason.message, "error");
				}
			});

			// Refresh the installed mods list
			await refreshInstalledMods();

			// Show success message
			if (successful.length > 0) {
				addMessage(
					`Successfully updated ${successful.length} mod(s).`,
					"success",
				);
			}
		} catch (error) {
			console.error("Failed to update mods:", error);
			addMessage(
				`Update all failed: ${error instanceof Error ? error.message : String(error)}`,
				"error",
			);
		}
	}

	// Helper function for Steamodded installation (matching ModCard.svelte pattern)
	async function installModFromURL(
		mod: Mod,
		url: string,
		folder_name: string = "",
	) {
		const wasInstalled = Boolean($installationStatus[mod.title]);
		let desiredEnabledState = true;

		if (wasInstalled) {
			let previousEnabled = $modEnabledStore?.[mod.title];
			if (previousEnabled === undefined) {
				try {
					previousEnabled = await invoke<boolean>("is_mod_enabled", {
						modName: mod.title,
					});
				} catch (error) {
					console.error(
						`Failed to read existing enabled state for ${mod.title}:`,
						error,
					);
				}
			}

			if (previousEnabled !== undefined) {
				desiredEnabledState = previousEnabled;
			}
		}

		try {
			if (!url.startsWith("http") && !url.startsWith("bmi://")) {
				console.error("Invalid URL format:", url);
				throw new Error(`Invalid URL format: ${url}`);
			}

			// Use mod title as fallback if folder_name is empty
			const folderName =
				folder_name || mod.folderName || mod.title.replace(/\s+/g, "");

			const installedPath = await invoke<string>("install_mod", {
				url,
				folderName,
			});

			await invoke("add_installed_mod", {
				name: mod.title,
				path: installedPath,
				dependencies: mod.requires_steamodded ? ["Steamodded"] : [],
				currentVersion: mod.version || "",
			});

			installationStatus.update((s) => ({ ...s, [mod.title]: true }));
			updateAvailableStore.update((s) => ({ ...s, [mod.title]: false }));

			modEnabledStore.update((s) => ({
				...s,
				[mod.title]: desiredEnabledState,
			}));
		} catch (error) {
			console.error("Failed to install mod:", error);
			throw error; // Rethrow to be handled by the caller
		}
	}

	const installMod = async (mod: Mod) => {
		if (!mod?.title || !mod?.downloadURL) return;

		// Define the actual download function that will be stored and executed later if needed
		const performDownload = async () => {
			try {
				loadingStates2.update((s: Record<string, boolean>) => ({
					...s,
					[mod.title]: true,
				}));

				// Create dependencies array for the database
				const dependencies = [];
				if (mod.requires_steamodded) dependencies.push("Steamodded");
				if (mod.requires_talisman) dependencies.push("Talisman");

				const installedPath = await invoke<string>("install_mod", {
					url: mod.downloadURL,
					folderName: mod.folderName || mod.title.replace(/\s+/g, ""),
				});

				await invoke("add_installed_mod", {
					name: mod.title,
					path: installedPath,
					dependencies,
					currentVersion: mod.version || "",
				});

				installationStatus.update((s) => ({ ...s, [mod.title]: true }));
				updateAvailableStore.update((s) => ({
					...s,
					[mod.title]: false,
				}));
				await refreshInstalledMods();
			} catch (error) {
				console.error("Failed to install mod:", error);
				const raw =
					error instanceof Error ? error.message : String(error);
				const onlyUrlMsg = raw.includes("Download URL not reachable")
					? raw.match(/Download URL not reachable[^"]*/)?.[0] || raw
					: `Installation failed: ${raw}`;
				addMessage(onlyUrlMsg, "error");
			} finally {
				loadingStates2.update((s: Record<string, boolean>) => ({
					...s,
					[mod.title]: false,
				}));
			}
		};

		try {
			// Check for dependencies
			if (mod.requires_steamodded || mod.requires_talisman) {
				// Check Steamodded if required
				const steamoddedInstalled = mod.requires_steamodded
					? await invoke<boolean>("check_mod_installation", {
							modType: "Steamodded",
						})
					: true;

				// Check Talisman if required
				const talismanInstalled = mod.requires_talisman
					? await invoke<boolean>("check_mod_installation", {
							modType: "Talisman",
						})
					: true;

				// If any dependency is missing, show the Requires Popup
				if (
					(mod.requires_steamodded && !steamoddedInstalled) ||
					(mod.requires_talisman && !talismanInstalled)
				) {
					// Call the handler with the appropriate requirements and download action
					handleDependencyCheck(
						{
							steamodded:
								mod.requires_steamodded && !steamoddedInstalled,
							talisman:
								mod.requires_talisman && !talismanInstalled,
						},
						performDownload,
					);
					return; // Stop installation
				}
			}

			// If we get here, either no dependencies are required or all are installed
			// Proceed with installation directly
			await performDownload();
		} catch (error) {
			console.error("Failed to check dependencies:", error);
			addMessage(
				`Dependency check failed: ${error instanceof Error ? error.message : String(error)}`,
				"error",
			);
		}
	};

	interface ModMeta {
		title: string;
		"requires-steamodded": boolean;
		"requires-talisman": boolean;
		categories: string[];
		author: string;
		repo: string;
		downloadURL?: string;
		folderName?: string;
		version?: string;
		"last-updated"?: number;
	}

	// Do not depend on cache for catalog; prefer fresh data + lazy UI
	const CACHE_DURATION = 0;

	// Types returned by the single-archive Tauri command
	interface ArchiveModItem {
		dir_name: string;
		meta: ModMeta;
		description: string;
		image_url: string;
	}

	async function refreshCatalogInBackground(showMessages: boolean = true): Promise<void> {
		if ($catalogLoading) return;
		catalogLoading.set(true);
		if (showMessages) {
			addMessage("Loading mods in background…", "info");
		}
		try {
			const items = await invoke<ArchiveModItem[]>("fetch_repo_mods", {
				sort: $currentSort,
			});
			const titles = items.map((i) => i.meta.title);
			const cachedMap = await invoke<Record<string, string>>(
				"get_cached_thumbnails_map",
				{ titles },
			);
			// Enqueue background caching for thumbnails (non-blocking, handles 429)
			try {
				const thumbItems = items
					.filter((i) => i.image_url && /^https?:\/\//i.test(i.image_url))
					.map((i) => ({ title: i.meta.title, url: i.image_url }));
				if (thumbItems.length > 0) {
					const toEnqueue = thumbItems.filter(
						(t) => !cachedMap[t.title],
					);
					if (toEnqueue.length > 0) {
						// fire and forget
						invoke("enqueue_thumbnails", { items: toEnqueue }).catch(
							() => {},
						);
					}
				}
			} catch (_) {
				/* ignore */
			}
            const mods: Mod[] = items.map((item) => {
				const mappedCategories = item.meta.categories
					.map((cat) => categoryMap[cat] ?? null)
					.filter((cat): cat is Category => cat !== null);

				const cachedThumb = cachedMap[item.meta.title];
				const img = cachedThumb ? convertFileSrc(cachedThumb) : "/images/cover.jpg";
				return {
					title: item.meta.title,
					description: item.description,
					image: img,
					imageFallback: cachedThumb ? img : undefined,
					colors: getRandomColorPair(),
					categories: mappedCategories,
					requires_steamodded: item.meta["requires-steamodded"],
					requires_talisman: item.meta["requires-talisman"],
					publisher: item.meta.author,
					repo: item.meta.repo,
					downloadURL: item.meta.downloadURL || "",
					folderName: item.meta.folderName,
					version: item.meta.version,
					installed: false,
					last_updated: item.meta["last-updated"] ?? 0,
					_dirName: item.dir_name,
				};
			});

            // Merge fresh remote mods with any locally seeded placeholders; keep server order
            const prunedCount = mergeIncomingMods(mods);

            if (prunedCount > 0) {
                addMessage(`Pruned ${prunedCount} removed mod${prunedCount === 1 ? '' : 's'} from cache`, "info");
            }

            // Persist refreshed upstream catalog to Rust cache for update checks
            try {
                const forCache = mods.map((m) => ({
                    title: m.title,
                    description: m.description,
                    image: m.image,
                    categories: m.categories,
                    colors: m.colors,
                    installed: false,
                    requires_steamodded: m.requires_steamodded,
                    requires_talisman: m.requires_talisman,
                    publisher: m.publisher,
                    repo: m.repo,
                    downloadURL: m.downloadURL || "",
                    folderName: m.folderName ?? null,
                    version: m.version ?? null,
                }));
                invoke("save_mods_cache", { mods: forCache }).catch(() => {});
            } catch (_) { /* ignore */ }

			// Re-apply local thumbnails for installed mods (non-blocking)
			fillInstalledThumbnails($modsStore).catch(() => {});
			// Re-check cached thumbnails after background fetches
			scheduleThumbCacheRefresh(titles);
			// Suspend cache persistence during description hydration to avoid thrashing localStorage
			await withModsCachePersistenceSuspended(async () => {
				await withDescriptionsPersistenceSuspended(async () => {
					try { await fillCachedDescriptionsVisibleFirst(); } catch { /* ignore */ }
					try { await fillDescriptionsVisibleFirst(); } catch { /* ignore */ }
					fillCachedDescriptions($modsStore).catch(() => {});
					fillDescriptions(mods).catch((e) => console.warn("desc fill failed", e));
				});
			});
			if (showMessages) {
				addMessage("All mods loaded", "success");
			}
		} catch (error) {
			console.error("Failed to refresh catalog:", error);
			if (showMessages) {
				addMessage(
					`Background load failed: ${error instanceof Error ? error.message : String(error)}`,
					"error",
				);
			}
		} finally {
			catalogLoading.set(false);
		}
	}

	// Foreground loader for first-run (no cached catalog): blocks UI spinner until ready
	async function loadCatalogForeground(): Promise<void> {
		if ($catalogLoading) return;
		catalogLoading.set(true);
        try {
            const items = await invoke<ArchiveModItem[]>("fetch_repo_mods", {
                sort: $currentSort,
            });
            const titles = items.map((i) => i.meta.title);
            const cachedMap = await invoke<Record<string, string>>(
                "get_cached_thumbnails_map",
                { titles },
            );
            // Enqueue background caching for thumbnails
            try {
                const thumbItems = items
                    .filter((i) => i.image_url && /^https?:\/\//i.test(i.image_url))
                    .map((i) => ({ title: i.meta.title, url: i.image_url }));
                if (thumbItems.length > 0) {
                    const seen = new Set<string>();
                    const filtered = thumbItems.filter((t) => {
                        if (seen.has(t.title)) return false;
                        seen.add(t.title);
                        if (cachedMap[t.title]) return false;
                        const existing = $modsStore.find((m) => m.title === t.title);
                        if (existing && existing.image && !existing.image.endsWith("cover.jpg")) {
                            return false;
                        }
                        return true;
                    });
                    if (filtered.length > 0) {
                        invoke("enqueue_thumbnails", { items: filtered }).catch(() => {});
                    }
                }
            } catch (_) { /* ignore */ }
            const mods: Mod[] = items.map((item) => {
				const mappedCategories = item.meta.categories
					.map((cat) => categoryMap[cat] ?? null)
					.filter((cat): cat is Category => cat !== null);

				const cachedThumb = cachedMap[item.meta.title];
				const img = cachedThumb ? convertFileSrc(cachedThumb) : "/images/cover.jpg";
				return {
					title: item.meta.title,
					description: item.description,
					image: img,
					imageFallback: cachedThumb ? img : undefined,
					colors: getRandomColorPair(),
					categories: mappedCategories,
					requires_steamodded: item.meta["requires-steamodded"],
					requires_talisman: item.meta["requires-talisman"],
					publisher: item.meta.author,
					repo: item.meta.repo,
					downloadURL: item.meta.downloadURL || "",
					folderName: item.meta.folderName,
					version: item.meta.version,
					installed: false,
					last_updated: item.meta["last-updated"] ?? 0,
					_dirName: item.dir_name,
				};
			});

            // Merge with any pre-seeded placeholders, and cautiously prune removed mods
            const prunedCount = mergeIncomingMods(mods as Mod[]);

            if (prunedCount > 0) {
                addMessage(`Pruned ${prunedCount} removed mod${prunedCount === 1 ? '' : 's'} from cache`, "info");
            }

            // Persist refreshed upstream catalog to Rust cache
            try {
                const forCache = (mods as Mod[]).map((m) => ({
                    title: m.title,
                    description: m.description,
                    image: m.image,
                    categories: m.categories,
                    colors: m.colors,
                    installed: false,
                    requires_steamodded: m.requires_steamodded,
                    requires_talisman: m.requires_talisman,
                    publisher: m.publisher,
                    repo: m.repo,
                    downloadURL: m.downloadURL || "",
                    folderName: m.folderName ?? null,
                    version: m.version ?? null,
                }));
                invoke("save_mods_cache", { mods: forCache }).catch(() => {});
            } catch (_) { /* ignore */ }

			// Also kick off thumbnails/descriptions
			fillInstalledThumbnails($modsStore).catch(() => {});
			scheduleThumbCacheRefresh(titles);
			await withModsCachePersistenceSuspended(async () => {
				await withDescriptionsPersistenceSuspended(async () => {
					try { await fillCachedDescriptionsVisibleFirst(); } catch { /* ignore */ }
					try { await fillDescriptionsVisibleFirst(); } catch { /* ignore */ }
					fillCachedDescriptions($modsStore).catch(() => {});
					fillDescriptions(mods).catch(() => {});
				});
			});
		} finally {
			catalogLoading.set(false);
		}
	}

	async function fillDescriptions(mods: (Mod & { _dirName?: string })[]) {
		// Limit concurrent requests to avoid 429s and prioritize detail view
		const limit = 6;
		let i = 0;
		const updates: { title: string; description: string }[] = [];
		const applyBatch = () => {
			if (updates.length === 0) return;
			const batch = updates.splice(0, updates.length);
			setDescriptions(
				Object.fromEntries(batch.map((b) => [b.title, b.description])),
			);
		};
		async function worker() {
			while (true) {
				const idx = i++;
				if (idx >= mods.length) break;
				const m = mods[idx];
				if (!m || m.description) continue;
				if (attemptedDescriptions.has(m.title)) continue;
				if (inflightDescriptions.has(m.title)) continue;
                const dir = m._dirName as string | undefined;
				if (!dir) continue;
				try {
					inflightDescriptions.add(m.title);
					const text = await invoke<string>(
						"get_description_cached_or_remote",
						{ title: m.title, dirName: dir },
					);
					attemptedDescriptions.add(m.title);
					updates.push({ title: m.title, description: text });
				} catch (_) {
					// ignore per-mod desc failures
					attemptedDescriptions.add(m.title);
				} finally {
					inflightDescriptions.delete(m.title);
				}
			}
		}
		await Promise.all(
			new Array(Math.min(limit, mods.length)).fill(0).map(worker),
		);
		applyBatch();
	}

	async function fillDescriptionsVisibleFirst() {
		if (isUserScrolling) {
			hydrationPending = true;
			return;
		}
		// Prioritize current page mods so skeletons disappear quickly
		const candidates = paginatedMods
			.filter((m) => !m.description || m.description.trim().length === 0)
			.map((m) => ({ title: m.title, dir: m._dirName as string | undefined }))
			.filter((x) => Boolean(x.dir));
		if (candidates.length === 0) return;
		const limit = 4;
		let i = 0;
		const updates: { title: string; description: string }[] = [];
		const applyBatch = () => {
			if (updates.length === 0) return;
			const batch = updates.splice(0, updates.length);
			setDescriptions(
				Object.fromEntries(batch.map((b) => [b.title, b.description])),
			);
		};
		visibleFirstRunning = true;
		async function worker() {
			while (true) {
				const idx = i++;
				if (idx >= candidates.length) break;
				const c = candidates[idx]!;
				if (inflightDescriptions.has(c.title)) continue;
				if (attemptedDescriptions.has(c.title)) continue;
				try {
					inflightDescriptions.add(c.title);
					const text = await invoke<string>(
						"get_description_cached_or_remote",
						{ title: c.title, dirName: c.dir }
					);
					attemptedDescriptions.add(c.title);
					updates.push({ title: c.title, description: text });
				} catch (_) {
					// ignore
					attemptedDescriptions.add(c.title);
				} finally {
					inflightDescriptions.delete(c.title);
				}
			}
		}
		await Promise.all(
			new Array(Math.min(limit, candidates.length)).fill(0).map(() => worker()),
		);
		applyBatch();
		visibleFirstRunning = false;
	}

	async function fillCachedDescriptions(mods: Mod[]) {
		// Only reads local cache; no network. Gentle concurrency.
		const limit = 12;
		let i = 0;
		const updates: { title: string; description: string }[] = [];
		const applyBatch = () => {
			if (updates.length === 0) return;
			const batch = updates.splice(0, updates.length);
			setDescriptions(
				Object.fromEntries(batch.map((b) => [b.title, b.description])),
			);
		};
		async function worker() {
			while (true) {
				const idx = i++;
				if (idx >= mods.length) break;
				const m = mods[idx];
				if (!m || (m.description && m.description.trim().length > 0)) continue;
				try {
					const cached = await invoke<string | null>(
						"get_cached_description_by_title",
						{ title: m.title },
					);
                        if (cached) {
							updates.push({ title: m.title, description: cached });
                        }
				} catch (_) {
					// ignore
				}
			}
		}
		await Promise.all(
			new Array(Math.min(limit, mods.length)).fill(0).map(() => worker()),
		);
		applyBatch();
	}

	async function fillCachedDescriptionsVisibleFirst() {
		if (isUserScrolling) {
			hydrationPending = true;
			return;
		}
		const candidates = paginatedMods
			.filter((m) => !m.description || m.description.trim().length === 0)
			.map((m) => m.title);
		if (candidates.length === 0) return;
		const limit = 8;
		let i = 0;
		const updates: { title: string; description: string }[] = [];
		const applyBatch = () => {
			if (updates.length === 0) return;
			const batch = updates.splice(0, updates.length);
			setDescriptions(
				Object.fromEntries(batch.map((b) => [b.title, b.description])),
			);
		};
		async function worker() {
			while (true) {
				const idx = i++;
				if (idx >= candidates.length) break;
				const title = candidates[idx]!;
				if (attemptedCacheTitles.has(title)) continue;
				try {
					attemptedCacheTitles.add(title);
					const cached = await invoke<string | null>(
						"get_cached_description_by_title",
						{ title },
					);
                        if (cached) {
							updates.push({ title, description: cached });
                        }
				} catch (_) {
					// ignore
				}
			}
		}
		await Promise.all(
			new Array(Math.min(limit, candidates.length)).fill(0).map(() => worker()),
		);
		applyBatch();
	}

	async function fillInstalledThumbnails(mods: Mod[]) {
		const limit = 8;
		let i = 0;
		const client = async () => {
			while (true) {
				const idx = i++;
				if (idx >= mods.length) break;
				const m = mods[idx];
				if (!m) continue;
				if (!$installationStatus[m.title]) continue; // only for installed mods
				const dir = m._dirName as string | undefined;
				if (!dir) continue;
				try {
					const dataUrl = await invoke<string | null>(
						"get_cached_installed_thumbnail",
						{ title: m.title, dirName: dir },
					);
					if (dataUrl) {
						const resolved =
							dataUrl.startsWith("data:") || dataUrl.startsWith("http")
								? dataUrl
								: convertFileSrc(dataUrl);
						modsStore.update((arr) => {
                            const pos = arr.findIndex((x) => x.title === m.title);
                            if (pos >= 0) {
                                arr = arr.slice();
                                arr[pos] = { ...arr[pos], image: resolved, imageFallback: undefined };
                            }
							return arr;
						});
					}
				} catch (_) {
					// ignore per-mod failures
				}
			}
		};
		await Promise.all(
			new Array(Math.min(limit, mods.length)).fill(0).map(() => client()),
		);
	}

	function mergeIncomingMods(incomingMods: Mod[]): number {
		let prunedCount = 0;
		modsStore.update((arr) => {
			const incoming = new Map<string, Mod>();
			for (const m of incomingMods) incoming.set(m.title, m);
			const incomingOrder = incomingMods.map((m) => m.title);
			const existingRemoteCount = arr.reduce(
				(n, it) => n + (it._dirName ? 1 : 0),
				0,
			);
			const pruneAllowed =
				incoming.size > 0 &&
				incoming.size >= Math.max(10, Math.floor(existingRemoteCount * 0.5));
			const existingByTitle = new Map<string, Mod>(
				arr.map((m) => [m.title, m]),
			);
			const out: Mod[] = [];

			for (const title of incomingOrder) {
				const inc = incoming.get(title);
				if (!inc) continue;
				const existing = existingByTitle.get(title);
				if (existing) {
					const keepExistingImage =
						Boolean(existing.image) &&
						existing.image.trim().length > 0 &&
						!/\bimages\/cover\.jpg$/i.test(existing.image.trim());
					const preferExistingDesc =
						(existing.description?.trim().length ?? 0) > 0;
					out.push({
						...existing,
						...inc,
						description: preferExistingDesc
							? existing.description
							: (inc.description ?? ""),
						colors: existing.colors,
						image: keepExistingImage ? existing.image : inc.image,
						imageFallback: keepExistingImage
							? existing.imageFallback
							: inc.imageFallback,
					});
				} else {
					out.push(inc);
				}
			}

			for (const existing of arr) {
				if (incoming.has(existing.title)) continue;
				if (!existing._dirName) {
					out.push(existing);
				} else if (pruneAllowed) {
					prunedCount++;
				} else {
					out.push(existing);
				}
			}
			return out;
		});
		return prunedCount;
	}

	async function applyCachedThumbnails(titles: string[]) {
		if (!titles.length) return;
		try {
			const cachedMap = await invoke<Record<string, string>>(
				"get_cached_thumbnails_map",
				{ titles },
			);
			if (!cachedMap || Object.keys(cachedMap).length === 0) return;
			modsStore.update((arr) =>
				arr.map((m) => {
					const p = cachedMap[m.title];
					if (!p) return m;
					const src = convertFileSrc(p);
					return { ...m, image: src, imageFallback: src };
				}),
			);
		} catch (_) {
			/* ignore */
		}
	}

	function scheduleThumbCacheRefresh(titles: string[]) {
		if (!titles.length) return;
		setTimeout(() => {
			applyCachedThumbnails(titles).catch(() => {});
		}, 2000);
		setTimeout(() => {
			applyCachedThumbnails(titles).catch(() => {});
		}, 8000);
	}

	async function seedInstalledPlaceholders() {
		try {
			// Load installed mods quickly from DB cache helper
			installedMods = await fetchCachedMods();
			if (!installedMods || installedMods.length === 0) return;
			modsStore.update((arr) => {
				const existingTitles = new Set(arr.map((m) => m.title));
				const additions: Mod[] = installedMods
					.filter((m) => !existingTitles.has(m.name))
					.map(
						(m) =>
							({
								title: m.name,
								description: "",
								image: "/images/cover.jpg",
								colors: getRandomColorPair(),
								categories: [],
								requires_steamodded: false,
								requires_talisman: false,
								publisher: "Installed",
								repo: "",
								downloadURL: "",
								folderName: m.name,
								version: "",
								installed: true,
								last_updated: 0,
								// Keep private installed path for potential future local reads
								_installedPath: m.path,
							}),
					);
				return additions.length ? [...additions, ...arr] : arr;
			});

			// Immediately reflect installationStatus so filters show
			for (const m of installedMods) {
				installationStatus.update((s) => ({ ...s, [m.name]: true }));
			}
		} catch (e) {
			console.warn("seedInstalledPlaceholders failed", e);
		}
	}

	// No local clone or pull; we lazy-load from the repo instead.

	const categories = [
		{ name: "Installed Mods", icon: Download },
		{ name: "Search", icon: Search },
		{ name: "All Mods", icon: LayoutDashboard },
		{ name: "Content", icon: FolderHeart },
		{ name: "Miscellaneous", icon: BookOpen },
		{ name: "Joker", icon: Flame },
		{ name: "Quality of Life", icon: Star },
		{ name: "Technical", icon: Spade },
		{ name: "Resource Packs", icon: FolderHeart },
		{ name: "API", icon: Gamepad2 },
	];

	const colorPairs = [
		{ color1: "#4f6367", color2: "#425556" },
		{ color1: "#AA778D", color2: "#906577" },
		{ color1: "#A2615E", color2: "#89534F" },
		{ color1: "#A48447", color2: "#8B703C" },
		{ color1: "#4F7869", color2: "#436659" },
		{ color1: "#728DBF", color2: "#6177A3" },
		{ color1: "#5D5E8F", color2: "#4F4F78" },
		{ color1: "#796E9E", color2: "#655D86" },
		{ color1: "#64825D", color2: "#556E4E" },
		{ color1: "#86A367", color2: "#728A57" },
		{ color1: "#748C8A", color2: "#627775" },
	];

	const categoryMap: Record<string, Category> = {
		Content: Category.Content,
		content: Category.Content,
		Joker: Category.Joker,
		joker: Category.Joker,
		"Quality of Life": Category.QualityOfLife,
		"quality of life": Category.QualityOfLife,
		Technical: Category.Technical,
		technical: Category.Technical,
		Miscellaneous: Category.Miscellaneous,
		miscellaneous: Category.Miscellaneous,
		"Resource Packs": Category.ResourcePacks,
		"resource packs": Category.ResourcePacks,
		Resources: Category.ResourcePacks,
		resources: Category.ResourcePacks,
		API: Category.API,
		api: Category.API,
	};

	function getRandomColorPair() {
		return colorPairs[Math.floor(Math.random() * colorPairs.length)];
	}

	function handleModClick(mod: Mod) {
		currentModView.set(mod);
	}

	let showSearch = $derived($currentCategory === "Search");
	$currentCategory = "All Mods";

	let filteredMods = $derived($modsStore.filter((mod) => {
		switch ($currentCategory) {
			case "Content":
				return (
					mod.categories.includes(Category.Content) ||
					mod.categories.some((cat) => cat === 0) || // Assuming Content is enum value 0
					mod.title.toLowerCase().includes("content") ||
					(typeof mod.description === "string" &&
						mod.description.toLowerCase().includes("new content"))
				);
			case "Joker":
				return (
					mod.categories.includes(Category.Joker) ||
					mod.categories.some((cat) => cat === 1)
				);
			case "Quality of Life":
				return (
					mod.categories.includes(Category.QualityOfLife) ||
					mod.categories.some((cat) => cat === 2)
				);
			case "Technical":
				return (
					mod.categories.includes(Category.Technical) ||
					mod.categories.some((cat) => cat === 3)
				);
			case "Resource Packs":
				return (
					mod.categories.includes(Category.ResourcePacks) ||
					mod.categories.some((cat) => cat === 5)
				);
			case "API":
				return (
					mod.categories.includes(Category.API) ||
					mod.categories.some((cat) => cat === 6)
				);
			case "Miscellaneous":
				return (
					mod.categories.includes(Category.Miscellaneous) ||
					mod.categories.some((cat) => cat === 4)
				);
			case "Installed Mods":
				return Boolean($installationStatus[mod.title]);
			default:
				return true;
		}
	}));

	function handleCategoryClick(category: string) {
		currentPage.set(1);
		startPage = 1; // Reset sliding window
		currentCategory.set(category);
		scrollToTop();
		markPaginating();
		updateVirtualWindow();
	}

	function handleModsScroll() {
		isUserScrolling = true;
		if (scrollIdleTimer) clearTimeout(scrollIdleTimer);
		const delay = isLinux ? 240 : 160;
		scrollIdleTimer = window.setTimeout(() => {
			isUserScrolling = false;
			if (hydrationPending) scheduleHydration();
		}, delay);
		if (scrollRaf !== null) cancelAnimationFrame(scrollRaf);
		scrollRaf = requestAnimationFrame(() => {
			scrollRaf = null;
			updateVirtualWindow();
		});
	}

// Safely register global click handler with cleanup to avoid duplicates
let globalClickHandler: ((e: MouseEvent) => void) | null = null;
	onMount(() => {
	  globalClickHandler = (e: MouseEvent) => {
	    const target = e.target as HTMLElement;
	    const anchor = target?.closest?.("a");
	    if (anchor && anchor instanceof HTMLAnchorElement && anchor.href.startsWith("https://")) {
	      e.preventDefault();
	      openExternal(anchor.href);
	    }
	  };
	  document.addEventListener("click", globalClickHandler);
	});
onDestroy(() => {
  if (globalClickHandler) {
    document.removeEventListener("click", globalClickHandler);
    globalClickHandler = null;
  }
});

	function sortMods(mods: Mod[], _sortOption: SortOption): Mod[] {
		// Sorting is provided by the server; preserve incoming order.
		return mods;
	}

	// Add sort handler
	function handleSortChange(event: Event) {
		const select = event.target as HTMLSelectElement;
		currentSort.set(select.value as SortOption);
		// Derived values react to $currentSort; no manual assignment needed
		// Reset to first page when sort changes to prevent out-of-bounds issues
		if ($currentPage > 1) {
			currentPage.set(1);
			startPage = 1;
		}
		refreshCatalogInBackground(false).catch(() => {});
	}

    let sortedAndFilteredMods = $derived(sortMods(filteredMods, $currentSort));

    $effect(() => {
        // touch dependencies so effect runs when these change
        sortedAndFilteredMods;
        paginatedMods;
        if ($currentCategory === "Installed Mods") {
            updateEnabledDisabledLists();
        }
    });

    let totalPages = $derived(Math.ceil(sortedAndFilteredMods.length / $itemsPerPage));
	let paginatedMods = $derived(
        sortedAndFilteredMods.slice(
            ($currentPage - 1) * $itemsPerPage,
            $currentPage * $itemsPerPage,
        )
    );

	let visiblePaginatedMods: Mod[] = $state([]);

	function measureGrid() {
		if (!modsGridEl) return;
		const width = modsGridEl.clientWidth || 0;
		const estimatedCardWidth = Math.max(
			260,
			firstCardEl?.getBoundingClientRect().width || 280,
		);
		const cols = Math.max(
			1,
			Math.floor((width + gridGap) / (estimatedCardWidth + gridGap)),
		);
		virtualCols = cols;
		const measuredHeight = firstCardEl?.getBoundingClientRect().height;
		if (measuredHeight && measuredHeight > 0) {
			virtualRowHeight = Math.round(measuredHeight + gridGap);
		}
	}

	function updateVirtualWindow() {
		if ($currentCategory === "Installed Mods") {
			visiblePaginatedMods = paginatedMods;
			virtualPaddingTop = 0;
			virtualPaddingBottom = 0;
			return;
		}

		if (!modsScrollContainer) {
			visiblePaginatedMods = paginatedMods;
			virtualPaddingTop = 0;
			virtualPaddingBottom = 0;
			return;
		}

		measureGrid();

		const total = paginatedMods.length;
		if (total === 0) {
			visiblePaginatedMods = [];
			virtualPaddingTop = 0;
			virtualPaddingBottom = 0;
			return;
		}

		const rowHeight = Math.max(120, virtualRowHeight);
		const viewportHeight = modsScrollContainer.clientHeight || 800;
		const scrollTop = modsScrollContainer.scrollTop;
		const totalRows = Math.max(1, Math.ceil(total / virtualCols));
		const startRow = Math.max(
			0,
			Math.floor(scrollTop / rowHeight) - virtualOverscanRows,
		);
		const endRow = Math.min(
			totalRows,
			Math.ceil((scrollTop + viewportHeight) / rowHeight) + virtualOverscanRows,
		);
		const startIdx = startRow * virtualCols;
		const endIdx = Math.min(total, endRow * virtualCols);
		if (endIdx <= startIdx) {
			visiblePaginatedMods = paginatedMods.slice(
				0,
				Math.min(total, virtualCols),
			);
		} else {
			visiblePaginatedMods = paginatedMods.slice(startIdx, endIdx);
		}
		virtualPaddingTop = startRow * rowHeight;
		virtualPaddingBottom = Math.max(
			0,
			totalRows * rowHeight - endRow * rowHeight,
		);
	}

	function measureCell(node: HTMLElement, index: number) {
		if (index === 0) {
			firstCardEl = node;
			measureGrid();
			updateVirtualWindow();
		}
		return {
			update(newIndex: number) {
				if (newIndex === 0) {
					firstCardEl = node;
					measureGrid();
					updateVirtualWindow();
				} else if (firstCardEl === node) {
					firstCardEl = null;
				}
			},
			destroy() {
				if (firstCardEl === node) {
					firstCardEl = null;
				}
			},
		};
	}

// Whenever the visible page changes, try to quickly hydrate from cache and recalc window
$effect(() => {
    // touch dependency
    paginatedMods;
    const localMax = Math.max(
        enabledLocalMods.length,
        disabledLocalMods.length,
    );
    renderLimitLocal = Math.min(60, localMax || 60);
    if (observerLocal && localSentinel) observerLocal.observe(localSentinel);
    updateVirtualWindow();
    scheduleHydration();
});

onMount(() => {
	if (typeof ResizeObserver !== "undefined") {
		resizeObs = new ResizeObserver(() => {
			measureGrid();
			updateVirtualWindow();
		});
		if (modsScrollContainer) {
			resizeObs.observe(modsScrollContainer);
		}
	}
});

$effect(() => {
	if (!resizeObs || !modsScrollContainer) return;
	resizeObs.disconnect();
	resizeObs.observe(modsScrollContainer);
});

$effect(() => {
	modsGridEl;
	measureGrid();
	updateVirtualWindow();
});

onDestroy(() => {
    if (visibleHydrateTimer !== null) {
        clearTimeout(visibleHydrateTimer);
        visibleHydrateTimer = null;
    }
	if (resizeObs) {
		try {
			resizeObs.disconnect();
		} catch (_) {
			/* ignore */
		}
	}
});

	const maxVisiblePages = 5;
	let startPage = $state(1);

	let visibleEnabledLocal = $derived(
		enabledLocalMods.slice(0, renderLimitLocal),
	);
	let visibleDisabledLocal = $derived(
		disabledLocalMods.slice(0, renderLimitLocal),
	);
	let observerLocal: IntersectionObserver | null = null;

	function scheduleHydration() {
		hydrationPending = true;
		if (paginating || isUserScrolling) return;
		if (visibleHydrateTimer !== null) {
			clearTimeout(visibleHydrateTimer);
		}
		const delay = isLinux ? 160 : 120;
		visibleHydrateTimer = setTimeout(() => {
			hydrationPending = false;
			void withDescriptionsPersistenceSuspended(async () => {
				await fillCachedDescriptionsVisibleFirst().catch(() => {});
				if (!visibleFirstRunning) {
					await fillDescriptionsVisibleFirst().catch(() => {});
				}
			});
		}, delay) as unknown as number;
	}

	function updatePaginationWindow() {
		if ($currentPage > startPage + maxVisiblePages - 1) {
			startPage = $currentPage - maxVisiblePages + 1;
		} else if ($currentPage < startPage) {
			startPage = $currentPage;
		}
	}

	async function refreshInstalledMods() {
		try {
			await forceRefreshCache();
			await refreshStateSummary();
		} catch (error) {
			console.error("Failed to refresh installed mods:", error);
		}
	}

	async function refreshStateSummary() {
		try {
			const localPaths = localMods.map((m) => m.path);
			const summary = await invoke<{
				installed: { name: string; path: string }[];
				enabled: Record<string, boolean>;
				updates: Record<string, boolean>;
				thumbnails: Record<string, string>;
				descriptions: Record<string, string>;
			}>("mods_state_summary", {
				localPaths,
				catalogTitles: paginatedMods.map((m) => m.title),
			});

			installedMods = summary.installed.map((m) => ({
				name: m.name,
				path: m.path,
			}));

			const installedSet = new Set(
				summary.installed.map((m) => m.name.toLowerCase()),
			);
			installationStatus.set(
				Object.fromEntries(
					$modsStore.map((mod) => [
						mod.title,
						installedSet.has(mod.title.toLowerCase()),
					]),
				),
			);
			modEnabledStore.set(summary.enabled || {});
			updateAvailableStore.set(summary.updates || {});
			// Apply cached thumbnails for installed mods
			const thumbMap = summary.thumbnails || {};
			if (Object.keys(thumbMap).length > 0) {
				modsStore.update((arr) =>
					arr.map((m) => {
						const p = thumbMap[m.title];
						if (p) {
							return {
								...m,
								image: convertFileSrc(p),
								imageFallback: convertFileSrc(p),
							};
						}
						return m;
					}),
				);
			}
			if (summary.descriptions && Object.keys(summary.descriptions).length > 0) {
				setDescriptions(summary.descriptions);
			}
			updateEnabledDisabledLists();
		} catch (error) {
			console.warn("Failed to refresh state summary:", error);
		}
	} 

	async function openModsFolder() {
		try {
			const modsPath: string = await invoke("get_mods_folder");
			await invoke("open_directory", { path: modsPath });
		} catch (error) {
			addMessage(`Failed to open mods directory: ${error}`, "error");
		}
	}

	let prevCategory = "";
	$effect(() => {
		const cat = $currentCategory;
		if (
			$currentModView === null &&
			cat === "Installed Mods" &&
			prevCategory !== "Installed Mods"
		) {
			// Category just switched to Installed Mods
			refreshInstalledMods();
			if (sortedAndFilteredMods.length === 0) {
				seedInstalledPlaceholders();
			}
		}
		prevCategory = cat;
	});

	$effect(() => {
		if (
			$modEnabledStore &&
			Object.keys($modEnabledStore).length > 0 &&
			$currentCategory === "Installed Mods"
		) {
			updateEnabledDisabledLists();
		}
	});
</script>

<div class="container default-scrollbar">
	<div class="mods-container">
		<div class="categories">
		{#each categories as category}
				{@const Icon = category.icon}
				<button
					class:active={$currentCategory === category.name}
					onclick={() => handleCategoryClick(category.name)}
				>
					<Icon size={16} />
					{category.name}
				</button>
			{/each}
		</div>

		<div class="separator"></div>

		{#if isLoading && $currentCategory !== "Installed Mods"}
			<div class="loading-container">
				<p class="loading-text">
					Loading mods{".".repeat($loadingDots)}
				</p>
			</div>
		{:else if showSearch}
			{#if SearchViewComp}
				<SearchViewComp onCheckDependencies={handleDependencyCheck} />
			{:else}
				<div class="loading-container">
					<p class="loading-text">Loading search…</p>
				</div>
			{/if}
		{:else}
			<div class="mods-wrapper">
				<div class="controls-container">
					{#if $currentCategory === "Installed Mods" && !$currentModView}
						<button
							class="folder-icon-button"
							onclick={openModsFolder}
							title="Open Mods Folder"
							in:fly={{ duration: 400, y: 10, opacity: 0.2 }}
						>
							<Folder size={20} />
						</button>

						{#if hasUpdatesAvailable}
							<button
								class="update-all-button-top"
								onclick={updateAllMods}
								title="Update all mods with available updates"
								in:fly={{ duration: 400, y: 10, opacity: 0.2 }}
							>
								<RefreshCw size={18} /> <span>Update All</span>
							</button>
						{/if}
					{/if}

					<div
						class="pagination-controls"
						in:fly={{ duration: 400, y: 10, opacity: 0.2 }}
					>
						<button
							onclick={previousPage}
							disabled={$currentPage === 1}>Previous</button
						>
						{#each Array(Math.min(maxVisiblePages, totalPages)) as _, i}
							{#if startPage + i <= totalPages}
								<button
									class:active={$currentPage ===
										startPage + i}
									onclick={() => goToPage(startPage + i)}
								>
									{startPage + i}
								</button>
							{/if}
						{/each}
						<button
							onclick={nextPage}
							disabled={$currentPage === totalPages}>Next</button
						>
					</div>

					<div
						class="sort-controls"
						in:fly={{ duration: 400, y: 10, opacity: 0.2 }}
					>
						<div class="sort-wrapper">
							<ArrowUpDown size={16} />
							<select
								value={$currentSort}
								onchange={handleSortChange}
							>
								<option value={SortOption.NameAsc}
									>Name (A-Z)</option
								>
								<option value={SortOption.NameDesc}
									>Name (Z-A)</option
								>
								<option value={SortOption.LastUpdatedDesc}
									>Last Updated</option
								>
								<option value={SortOption.LastUpdatedAsc}
									>Oldest Updated</option
								>
								<option value={SortOption.DownloadsDesc}
									>Downloads (Most)</option
								>
								<option value={SortOption.DownloadsAsc}
									>Downloads (Least)</option
								>
							</select>
						</div>
					</div>
				</div>

				<div
					class="mods-scroll-container default-scrollbar"
					class:no-local-mods={localMods.length === 0}
					bind:this={modsScrollContainer}
					onscroll={handleModsScroll}
				>
					{#if $currentCategory === "Installed Mods"}
						{#if isLoadingLocalMods}
							<div class="section-header">
								<h3>Local Mods</h3>
								<p>
									Loading local mods{".".repeat($loadingDots)}
								</p>
							</div>
						{:else if localMods.length > 0}
							<div class="section-header">
								<div class="section-header-content">
									<h3>Local Mods</h3>
									<p>
										These mods were installed manually
										(outside the mod manager)
									</p>
								</div>
								<!-- Removed Open Mods Folder button for Local Mods section -->
							</div>

							<!-- Enabled Local Mods -->
							{#if enabledLocalMods.length > 0}
								<div
									class="subsection-header enabled"
									class:top-margin={localMods.length === 0}
								>
									<h4>Enabled Local Mods</h4>
									<p>
										{enabledLocalMods.length} mod{enabledLocalMods.length !==
										1
											? "s"
											: ""} active
									</p>
								</div>
								<div class="mods-grid local-mods-grid">
									{#each visibleEnabledLocal as mod (mod.name)}
										<LocalModCard
											{mod}
											onUninstall={handleModUninstalled}
											onToggleEnabled={handleModToggled}
										/>
									{/each}
									<div
										bind:this={localSentinel}
										class="render-sentinel"
										aria-hidden="true"
									></div>
								</div>
							{/if}

							<!-- Disabled Local Mods -->
							{#if disabledLocalMods.length > 0}
								<div
									class="subsection-header disabled"
									class:top-margin={localMods.length === 0}
								>
									<h4>Disabled Local Mods</h4>
									<p>
										{disabledLocalMods.length} mod{disabledLocalMods.length !==
										1
											? "s"
											: ""} inactive
									</p>
								</div>
								<div class="mods-grid local-mods-grid">
									{#each visibleDisabledLocal as mod (mod.name)}
										<LocalModCard
											{mod}
											onUninstall={handleModUninstalled}
											onToggleEnabled={handleModToggled}
										/>
									{/each}
									<div
										bind:this={localSentinel}
										class="render-sentinel"
										aria-hidden="true"
									></div>
								</div>
							{/if}

							<!-- Mod Manager Catalog Section Header -->
							<div class="section-header">
								<div class="section-header-content">
									<h3>Mod Manager Catalog</h3>
									<p>
										These mods are available from the online
										catalog
									</p>
								</div>
								<!-- Removed Open Mods Folder button for Mod Manager Catalog section -->
							</div>
						{:else if !isLoadingLocalMods && localMods.length === 0 && paginatedMods.length === 0}
							<div class="no-mods-message">
								<p>No installed mods.</p>
								<div class="no-mods-buttons">
									<button
										class="open-folder-button"
										onclick={openModsFolder}
										title="Open mods folder"
									>
										<Folder size={20} /> Open Mods Folder
									</button>
								</div>
							</div>
						{/if}

						<!-- Only proceed with catalog enabled/disabled sections if there are mods to show -->
						{#if paginatedMods.length > 0}
							<!-- Enabled Catalog Mods -->
							{#if enabledMods.length > 0}
								<div class="subsection-header enabled">
									<h4>Enabled Catalog Mods</h4>
									<p>
										{enabledMods.length} mod{enabledMods.length !==
										1
											? "s"
											: ""} active
									</p>
								</div>
								<div
									class="mods-grid"
									class:has-local-mods={localMods.length > 0}
									bind:this={modsGridEl}
								>
									<div
										class="virtual-spacer"
										style={`height:${virtualPaddingTop}px`}
										aria-hidden="true"
									></div>
									{#each visiblePaginatedMods.filter((m) =>
										enabledMods.some((e) => e.title === m.title)
									) as mod, index (mod.title)}
										<div class="virtual-cell" use:measureCell={index}>
											<ModCard
												{mod}
												deferImages={paginating}
												onmodclick={handleModClick}
												oninstallclick={installMod}
												onuninstallclick={uninstallMod}
												onToggleEnabled={handleModToggled}
											/>
										</div>
									{/each}
									<div
										class="virtual-spacer"
										style={`height:${virtualPaddingBottom}px`}
										aria-hidden="true"
									></div>
								</div>
							{/if}

							<!-- Disabled Catalog Mods -->
							{#if disabledMods.length > 0}
								<div class="subsection-header disabled">
									<h4>Disabled Catalog Mods</h4>
									<p>
										{disabledMods.length} mod{disabledMods.length !==
										1
											? "s"
											: ""} inactive
									</p>
								</div>
								<div
									class="mods-grid"
									class:has-local-mods={localMods.length > 0}
									bind:this={modsGridEl}
								>
									<div
										class="virtual-spacer"
										style={`height:${virtualPaddingTop}px`}
										aria-hidden="true"
									></div>
									{#each visiblePaginatedMods.filter((m) =>
										disabledMods.some((e) => e.title === m.title)
									) as mod, index (mod.title)}
										<div class="virtual-cell" use:measureCell={index}>
											<ModCard
												{mod}
												deferImages={paginating}
												onmodclick={handleModClick}
												oninstallclick={installMod}
												onuninstallclick={uninstallMod}
												onToggleEnabled={handleModToggled}
											/>
										</div>
									{/each}
									<div
										class="virtual-spacer"
										style={`height:${virtualPaddingBottom}px`}
										aria-hidden="true"
									></div>
								</div>
							{/if}

							{#if enabledMods.length === 0 && disabledMods.length === 0}
								<!-- Fallback: show installed catalog mods before enabled state resolves -->
								<div class="mods-grid" bind:this={modsGridEl}>
									<div
										class="virtual-spacer"
										style={`height:${virtualPaddingTop}px`}
										aria-hidden="true"
									></div>
									{#each visiblePaginatedMods as mod, index (mod.title)}
										<div class="virtual-cell" use:measureCell={index}>
											<ModCard
												{mod}
												deferImages={paginating}
												onmodclick={handleModClick}
												oninstallclick={installMod}
												onuninstallclick={uninstallMod}
												onToggleEnabled={handleModToggled}
											/>
										</div>
									{/each}
									<div
										class="virtual-spacer"
										style={`height:${virtualPaddingBottom}px`}
										aria-hidden="true"
									></div>
								</div>
							{/if}
						{/if}
					{:else}
						<!-- Original non-InstalledMods categories -->
						<div class="mods-grid" bind:this={modsGridEl}>
							<div
								class="virtual-spacer"
								style={`height:${virtualPaddingTop}px`}
								aria-hidden="true"
							></div>
							{#each visiblePaginatedMods as mod, index (mod.title)}
								<div class="virtual-cell" use:measureCell={index}>
									<ModCard
										{mod}
										deferImages={paginating}
										onmodclick={handleModClick}
										oninstallclick={installMod}
										onuninstallclick={uninstallMod}
									/>
								</div>
							{/each}
							<div
								class="virtual-spacer"
								style={`height:${virtualPaddingBottom}px`}
								aria-hidden="true"
							></div>
						</div>
					{/if}
				</div>
			</div>
		{/if}
	</div>

	{#if $currentModView}
		<ModView
			mod={$currentModView!}
			onCheckDependencies={handleDependencyCheck}
		/>
	{/if}
</div>

<style>
	.container.default-scrollbar {
		position: relative;
	}

	.update-all-button-top {
		position: absolute;
		top: 50%;
		left: 2.5rem; /* Position it next to the folder button */
		transform: translateY(-50%);
		z-index: 3000;
		background: #3498db;
		color: #f4eee0;
		border: 2px solid #f4eee0;
		border-radius: 8px;
		height: 47px;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition: all 0.2s ease;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
		padding: 0 1rem;
		font-family: "M6X11", sans-serif;
		font-size: 0.9rem;
		white-space: nowrap;
		gap: 0.5rem;
	}

	.update-all-button-top:hover {
		background: #2980b9;
		transform: translateY(-50%) scale(1.1);
	}

	.update-all-button-top:active {
		transform: translateY(-50%) scale(0.95);
	}

	/* Adjust position for smaller screens */
	@media (max-width: 1160px) {
		.update-all-button-top {
			left: 2.2rem;
		}
	}

	.no-mods-buttons {
		display: flex;
		gap: 0.75rem;
		justify-content: center;
		flex-wrap: wrap;
	}

	.section-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		flex-wrap: wrap;
		gap: 1rem;
	}

	.section-header-content {
		flex: 1;
		min-width: 200px;
	}

	.subsection-header {
		display: flex;
		flex-direction: column;
		background: #4f6367;
		border: 2px solid #f4eee0; /*Full white border like section header*/
		padding: 0.7rem 1.5rem;
		margin: 0 2rem 1rem 2rem;
		border-radius: 8px; /*Matching border-radius*/
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3); /*Matching box-shadow*/
	}

	.subsection-header.enabled {
		background: #27ae60;
		border: 2px solid #f4eee0;
	}

	.subsection-header.disabled {
		background: #7f8c8d;
		border: 2px solid #f4eee0;
	}

	.subsection-header h4 {
		margin: 0;
		font-size: 1.3rem;
		color: #f4eee0;
		text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.5);
	}

	.subsection-header p {
		margin: 0.2rem 0 0 0;
		font-size: 1rem;
		color: #f4eee0;
		opacity: 0.9;
	}

	/*Adjustments for grid spacing when using subsections*/
	.mods-grid {
		padding-top: 0.5rem;
	}

	.mods-grid:last-child {
		padding-bottom: 2rem;
	}

	.folder-icon-button {
		position: absolute;
		top: 50%;
		left: -0.2rem; /* Nudge right to avoid clipping */
		transform: translateY(-50%);
		z-index: 3000;
		background: #4caf50;
		color: #f4eee0;
		border: 2px solid #f4eee0;
		border-radius: 8px;
		width: 52px;
		height: 47px;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition: all 0.2s ease;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
		padding: 0;
	}

	.folder-icon-button:hover {
		background: #45a049;
		transform: translateY(-50%) scale(1.1);
	}

	.folder-icon-button:active {
		transform: translateY(-50%) scale(0.95);
	}

	/*Adjust position for smaller screens*/
	@media (max-width: 1160px) {
		.folder-icon-button {
			left: -0.6rem;
		}
	}

	.open-folder-button {
		background: #4caf50;
		color: #f4eee0;
		border: none;
		outline: #3d8b40 solid 2px;
		border-radius: 4px;
		padding: 0.75rem 1.5rem;
		font-family: "M6X11", sans-serif;
		font-size: 1.2rem;
		cursor: pointer;
		transition: all 0.2s ease;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.open-folder-button:hover {
		background: #45a049;
		transform: translateY(-2px);
	}

	.render-sentinel {
		width: 100%;
		height: 1px;
	}

	.virtual-spacer {
		grid-column: 1 / -1;
		width: 100%;
		pointer-events: none;
	}

	.virtual-cell {
		min-height: 0;
	}

	.open-folder-button:active {
		transform: translateY(1px);
	}

	.section-header {
		background: #c14139;
		border: 2px solid #f4eee0;
		border-radius: 8px;
		padding: 1rem 2rem;
		margin: 0 2rem 1rem 2rem;
		margin-top: 2rem;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}

	.section-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		flex-wrap: wrap;
		gap: 1rem;
	}

	.section-header-content {
		flex: 1;
	}

	.section-header h3 {
		margin: 0;
		font-size: 1.8rem;
		color: #f4eee0;
		text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.5);
	}

	.section-header p {
		margin: 0.5rem 0 0 0;
		font-size: 1.1rem;
		color: #f4eee0;
	}

	.mods-container {
		display: flex;
		gap: 1rem;
		padding: 0 2rem;
		overflow: hidden;
		height: 100%;
		contain: layout paint;
	}

	.no-mods-message {
		display: flex;
		justify-content: center;
		flex-direction: column;
		align-items: center;
		height: 100%;
		width: 100%;
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		margin: auto;
		padding-top: 3rem; /*Add space for the controls at the top*/
	}

	.no-mods-message p {
		font-family: "M6X11", sans-serif;
		font-size: 1.8rem;
		color: #f4eee0;
		text-align: center;
		/*Add black stroke with two methods for better browser compatibility*/
		-webkit-text-stroke: 0.1px black;
		/*Fallback using text-shadow for browsers that don't support text-stroke*/
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000,
			2px 2px 3px rgba(0, 0, 0, 0.5);
	}

	.separator {
		width: 2px;
		background: #f4eee0;
		height: 100%;
	}

	.pagination-controls {
		position: absolute;
		/*top: 0.05rem;*/
		left: 50%;
		transform: translateX(-50%);
		z-index: 1000;
		background: #c14139;
		border: 2px solid #f4eee0;
		border-radius: 8px;
		padding: 0.5rem 1rem;
		display: flex;
		gap: 0.5rem;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}

	.pagination-controls button {
		padding: 0.5rem 1rem;
		background: #ea9600;
		border: 2px solid #f4eee0;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 0.8rem;
		cursor: pointer;
		border-radius: 4px;
		transition: all 0.2s ease;
	}

	.pagination-controls button:hover:not(:disabled) {
		background: #f4eee0;
		color: #393646;
	}

	.pagination-controls button.active {
		background: #f4eee0;
		color: #393646;
	}

	.pagination-controls button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.controls-container {
		height: 75px;
		width: 100%;
		display: flex;
		position: absolute;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
	}

	.categories {
		width: 190px;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		overflow-y: auto;
		padding: 2rem 0;
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
		scrollbar-width: 0;
		scrollbar-color: transparent transparent;
	}

	.categories button {
		text-align: left;
		padding: 1rem 1rem;
		background: #ea9600;
		border: 2px solid #f4eee0;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		cursor: pointer;
		transition: all 0.2s ease;
		border-radius: 6px;
		margin-right: 0.3rem;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.categories button:hover {
		background: #f4eee0;
		color: #393646;
	}

	.categories button.active {
		background: #f4eee0;
		color: #393646;
	}

	.mods-scroll-container {
		overflow-y: auto;
		height: 100%;
		contain: layout paint;
		scrollbar-gutter: stable;
		backface-visibility: hidden;
		transform: translateZ(0);
		will-change: scroll-position;
		overscroll-behavior: contain;
	}

	.mods-scroll-container.no-local-mods .subsection-header:first-of-type {
		margin-top: 3rem; /*Add spacing at the top when there are no local mods*/
	}

	.top-margin {
		margin-top: 3rem !important;
	}

    .mods-grid {
		padding: 1rem 2rem 2rem 2rem;
		flex: 1;
		display: grid;
		grid-template-columns: repeat(
			auto-fill,
			minmax(calc(280px * var(--card-scale, 1)), 1fr)
		);
		gap: 30px;
		content-visibility: auto;
		contain-intrinsic-size: 900px 1200px;
	}

	.local-mods-grid {
		padding-top: 0.5rem;
		padding-bottom: 1rem;
	}

	.sort-controls {
		position: absolute;
		/*top: 0.25rem; Increased from 2rem*/
		right: 4rem; /*Increased from 2.5rem*/
		z-index: 1000;
		margin: 0;
		background: transparent;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
		/*transform: translateY(0); /*Reset any transforms*/
	}
	/**/
	/*.sort-controls {*/
	/*position: absolute;*/
	/*top: 1rem;*/
	/*right: 3rem;*/
	/*z-index: 1000;*/
	/*margin: 0;*/
	/*background: transparent;*/
	/*}*/

	.sort-wrapper {
		background: #ea9600;
		border: 2px solid #f4eee0;
		padding: 0.5rem;
		border-radius: 6px;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		transition: all 0.2s ease;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
	}

	.mods-wrapper {
		position: relative;
		/*192px being the width of the catagories + seperator*/
		width: calc(100% - 192px);
		padding: 0 1rem;
		contain: layout paint;
	}

	.sort-wrapper :global(svg) {
		color: #f4eee0;
	}

	select {
		background: #ea9600;
		color: #f4eee0;
		border: none;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		padding: 0.25rem 1.5rem 0.25rem 0.5rem;
		border-radius: 4px;
		cursor: pointer;
		-webkit-appearance: none;
		-moz-appearance: none;
		appearance: none;
		background-image: url("data:image/svg+xml;charset=US-ASCII,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%22292.4%22%20height%3D%22292.4%22%3E%3Cpath%20fill%3D%22%23F4EEE0%22%20d%3D%22M287%2069.4a17.6%2017.6%200%200%200-13-5.4H18.4c-5%200-9.3%201.8-12.9%205.4A17.6%2017.6%200%200%200%200%2082.2c0%205%201.8%209.3%205.4%2012.9l128%20127.9c3.6%203.6%207.8%205.4%2012.8%205.4s9.2-1.8%2012.8-5.4L287%2095c3.5-3.5%205.4-7.8%205.4-12.8%200-5-1.9-9.2-5.4-12.8z%22%2F%3E%3C%2Fsvg%3E");
		background-repeat: no-repeat;
		background-position: right 0.5em top 50%;
		background-size: 0.65em auto;
	}

	select:hover {
		background-color: #f0a620;
	}

	select:focus {
		outline: none;
		box-shadow: 0 0 0 2px #f4eee0;
	}

	select option {
		background: #ea9600;
		color: #f4eee0;
		padding: 0.5rem;
	}

	.sort-wrapper:hover {
		transform: translateY(-1px);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
	}

	.loading-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		flex: 1;
	}

	.loading-text {
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 1.5rem;
		min-width: 150px;
	}

	@media (max-width: 1160px) {
		.pagination-controls button {
			min-width: 3rem;
			padding: 0.4rem 0.6rem;
			font-size: 0.75rem;
		}

		.pagination-controls {
			left: 20rem;
		}

		.controls-container {
			margin-bottom: 0.5rem;
		}

		.sort-controls {
			right: 1rem;
		}
	}
</style>
