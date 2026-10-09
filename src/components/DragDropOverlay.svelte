<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { addMessage } from "$lib/stores";
  import { webviewWindow } from "@tauri-apps/api";
  import { Archive } from "lucide-svelte";

  let isProcessing = false;
  let isDragging = false;
  let unlisten: (() => void) | null = null;

  // Use Tauri's inferred event types from `onDragDropEvent`.

  onMount(async () => {
    try {
      // Get current webview window
      const webview = webviewWindow.getCurrentWebviewWindow();

      // Listen for file drop events
      unlisten = await webview.onDragDropEvent(async (event) => {
        // Handle different event types per Tauri v2 API
        if (event.payload.type === "over" || event.payload.type === "enter") {
          isDragging = true;
        } else if (event.payload.type === "leave") {
          isDragging = false;
        } else if (event.payload.type === "drop") {
          // Always hide the drag overlay on drop
          isDragging = false;

          // Paths are provided in event.payload.paths per Tauri v2
          const paths = event.payload.paths;

          if (!paths || paths.length === 0) {
            return;
          }

          isProcessing = true;

          // Process each dropped file
          for (const filePath of paths) {
            try {
              // Check if it's a supported file type
              if (
                !filePath.endsWith(".zip") &&
                !filePath.endsWith(".tar") &&
                !filePath.endsWith(".tar.gz") &&
                !filePath.endsWith(".tgz") &&
                !filePath.endsWith(".rar") &&
                !filePath.endsWith(".7z")
              ) {
                addMessage(
                  `Skipped ${filePath}: Only ZIP, TAR, TAR.GZ, RAR, and 7Z archives are supported`,
                  "warning",
                );
                continue;
              }

              // Process the file
              try {
                await invoke("process_dropped_file", {
                  path: filePath,
                });

                addMessage(
                  `Successfully installed mod from: ${filePath}`,
                  "success",
                );

                // Refresh the mods list
                await invoke("reindex_mods");
              } catch (invokeError) {
                console.error("Invoke error:", invokeError);
                addMessage(`Error processing file: ${invokeError}`, "error");
              }
            } catch (error) {
              console.error("Error processing file:", filePath, error);
              addMessage(`Failed to process ${filePath}: ${error}`, "error");
            }
          }

          isProcessing = false;
        }
      });
    } catch (error) {
      console.error("Error setting up drag-drop handler:", error);
      addMessage(`Error setting up drag-drop handler: ${error}`, "error");
    }
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });
</script>

{#if isDragging}
  <div
    class="drag-drop-overlay"
    role="region"
    aria-label="Drop zone for mod files"
  >
    <div class="drop-zone">
      <Archive size={64} color="#fdcf51" />
      <h2>Drop Mod Files Here</h2>
      <p>Drop ZIP, TAR, TAR.GZ, RAR, or 7Z files to install mods</p>
    </div>
  </div>
{/if}

{#if isProcessing}
  <div class="processing-overlay">
    <div class="spinner"></div>
    <p>Installing mod...</p>
  </div>
{/if}

<style>
  /* Styles unchanged */
  .drag-drop-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
    animation: fadeIn 0.2s ease-in-out;
    pointer-events: none; /* This ensures the overlay doesn't intercept events */
  }

  .drop-zone {
    background-color: rgba(253, 207, 81, 0.1);
    border: 3px dashed #fdcf51;
    border-radius: 16px;
    padding: 3rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    color: #f4eee0;
    width: 80%;
    max-width: 500px;
    height: 300px;
  }

  h2 {
    font-size: 2rem;
    margin: 1rem 0;
    color: #fdcf51;
  }

  p {
    font-size: 1.2rem;
    opacity: 0.8;
  }

  .processing-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.7);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }

  .spinner {
    width: 50px;
    height: 50px;
    border: 5px solid #fdcf51;
    border-radius: 50%;
    border-top-color: transparent;
    animation: spin 1s linear infinite;
    margin-bottom: 1rem;
  }

  .processing-overlay p {
    color: #f4eee0;
    font-size: 1.5rem;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
