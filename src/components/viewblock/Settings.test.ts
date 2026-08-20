import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Settings from "./Settings.svelte";
import {
  cachedVersions,
  catalogLastRefreshed,
  currentJokerView,
  currentModView,
  modsStore,
  searchResults,
} from "../../stores/modStore";
import { descriptionsStore } from "../../stores/descriptions";

const invokeMock = vi.hoisted(() => vi.fn());
const addMessageMock = vi.hoisted(() => vi.fn());
const isFullscreenMock = vi.hoisted(() => vi.fn());
const setFullscreenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("$lib/stores", () => ({
  addMessage: (...args: unknown[]) => addMessageMock(...args),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: vi.fn().mockResolvedValue("windows"),
}));

vi.mock("@tauri-apps/api/window", () => ({
  Window: {
    getCurrent: () => ({
      isFullscreen: (...args: unknown[]) => isFullscreenMock(...args),
      setFullscreen: (...args: unknown[]) => setFullscreenMock(...args),
    }),
  },
}));

function createMockMod(title: string) {
  return {
    title,
    description: "desc",
    image: "img",
    categories: [],
    colors: { color1: "#000", color2: "#111" },
    requires_steamodded: false,
    requires_talisman: false,
    publisher: "pub",
    repo: "repo",
    downloadURL: "https://example.com",
    installed: true,
    last_updated: 0,
  };
}

describe("Settings view", () => {
  let mockStorage: Record<string, string> = {};

  beforeEach(() => {
    vi.clearAllMocks();
    isFullscreenMock.mockResolvedValue(true);
    setFullscreenMock.mockResolvedValue(undefined);
    mockStorage = {};
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => mockStorage[key] ?? null,
      setItem: (key: string, value: string) => {
        mockStorage[key] = value;
      },
      removeItem: (key: string) => {
        delete mockStorage[key];
      },
      clear: () => {
        mockStorage = {};
      },
      key: (i: number) => Object.keys(mockStorage)[i] ?? null,
      get length() {
        return Object.keys(mockStorage).length;
      },
    });
    modsStore.set([createMockMod("mod-a")]);
    searchResults.set([createMockMod("mod-b")]);
    currentModView.set(createMockMod("mod-c"));
    currentJokerView.set(createMockMod("mod-d"));
    catalogLastRefreshed.set(12345);
    cachedVersions.set({ steamodded: ["1.0.0"], talisman: ["2.0.0"] });
    descriptionsStore.set({ "mod-a": "cached description" });

    invokeMock.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "get_all_settings") {
        return {
          discord_rpc: false,
          lovely_console: false,
          background_enabled: false,
          compat_helper: true,
          linux_prefix: "",
          launch_mode: "modded",
          analytics_enabled: true,
        };
      }
      if (command === "get_balatro_path") {
        return "";
      }
      if (command === "get_mods_folder") {
        return "/tmp/mods";
      }
      if (command === "open_directory") {
        return args;
      }
      if (command === "reindex_mods") {
        return [0, 4];
      }
      if (command === "clear_cache") {
        return null;
      }
      return null;
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("opens the mods folder from Settings", async () => {
    render(Settings);

    await fireEvent.click(
      screen.getByRole("button", { name: /open mods folder/i }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_mods_folder");
      expect(invokeMock).toHaveBeenCalledWith("open_directory", {
        path: "/tmp/mods",
      });
    });
  });

  it("reindexes mod database and reports success", async () => {
    render(Settings);

    await fireEvent.click(
      screen.getByRole("button", { name: /validate mod database/i }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("reindex_mods");
      expect(addMessageMock).toHaveBeenCalledWith(
        "Reindex complete: Cleaned 4 database entries",
        "success",
      );
    });
  });

  it("clears caches and resets related in-memory stores", async () => {
    render(Settings);

    await fireEvent.click(screen.getByRole("button", { name: /clear cache/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("clear_cache");
      expect(addMessageMock).toHaveBeenCalledWith(
        "Successfully cleared all caches!",
        "success",
      );
    });

    expect(get(modsStore)).toEqual([]);
    expect(get(searchResults)).toEqual([]);
    expect(get(currentModView)).toBeNull();
    expect(get(currentJokerView)).toBeNull();
    expect(get(catalogLastRefreshed)).toBeNull();
    expect(get(cachedVersions)).toEqual({ steamodded: [], talisman: [] });
    expect(get(descriptionsStore)).toEqual({});
  });

  it("shows and toggles fullscreen state", async () => {
    render(Settings);

    const toggle = await screen.findByRole("checkbox", {
      name: /fullscreen/i,
    });
    await waitFor(() => {
      expect((toggle as HTMLInputElement).checked).toBe(true);
    });

    await fireEvent.click(toggle);

    await waitFor(() => {
      expect(setFullscreenMock).toHaveBeenCalledWith(false);
    });
    expect((toggle as HTMLInputElement).checked).toBe(false);
  });
});
