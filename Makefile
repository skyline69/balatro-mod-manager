# Define OS-specific variables
ifeq ($(OS),Windows_NT)
	REMOVE_TARGET =
	SET_SKIP_BUILD_SCRIPT = set SKIP_BUILD_SCRIPT=1
else
	SET_SKIP_BUILD_SCRIPT = SKIP_BUILD_SCRIPT=1
	CLEAR_SCREEN = clear
	MACOS_TARGET = MACOSX_DEPLOYMENT_TARGET=11.0
endif

# Debug target
debug:
	@$(CLEAR_SCREEN)
	@$(REMOVE_TARGET)
	@cargo tauri dev

# Release target
release:
	@$(CLEAR_SCREEN)
	@$(REMOVE_TARGET)
	@cargo tauri build --verbose

# Clean target
clean:
	@echo "Cleaning all build files..."
	@cd ./src-tauri && cargo clean
	@cd $(CRATE_DIR) && cargo clean

