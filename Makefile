# =============================================================================
# Tool Servers Makefile - Cross-compile MCP servers for multiple platforms
# =============================================================================

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------
SERVERS := bash elicitation filesystem plugins system terminal todolist web
TARGETS := darwin-arm64 linux-arm64 linux-x86_64

# Rust target triples
TARGET_darwin-arm64  := aarch64-apple-darwin
TARGET_linux-arm64   := aarch64-unknown-linux-gnu
TARGET_linux-x86_64  := x86_64-unknown-linux-gnu

# Native target (no cross needed)
NATIVE_TARGET := darwin-arm64

# Cross configuration file location
export CROSS_CONFIG := $(CURDIR)/Cross.toml

# -----------------------------------------------------------------------------
# Phony Targets
# -----------------------------------------------------------------------------
.PHONY: all build build-all clean help check-cross
.PHONY: $(foreach s,$(SERVERS),build-$(s))
.PHONY: $(foreach s,$(SERVERS),$(foreach t,$(TARGETS),build-$(s)-$(t)))

# -----------------------------------------------------------------------------
# Default Target
# -----------------------------------------------------------------------------
all: help

# -----------------------------------------------------------------------------
# Dependency Check
# -----------------------------------------------------------------------------
check-cross:
	@command -v cross >/dev/null 2>&1 || { \
		echo "Error: 'cross' is not installed."; \
		echo "Install with: cargo install cross"; \
		exit 1; \
	}

# -----------------------------------------------------------------------------
# Build Functions
# -----------------------------------------------------------------------------

# Build a single server for a single target
# Usage: $(call build-server,<server>,<target>)
# Output: packages/<server>/dist/<server>-<target>
define build-server
	@echo "Building $(1) for $(2)..."
	@mkdir -p packages/$(1)/dist
	$(if $(filter $(2),$(NATIVE_TARGET)), \
		cd packages/$(1) && cargo build --release --target $(TARGET_$(2)), \
		cd packages/$(1) && cross build --release --target $(TARGET_$(2)) \
	)
	@cp packages/$(1)/target/$(TARGET_$(2))/release/$(1) packages/$(1)/dist/$(1)-$(2)
	@echo "Built: packages/$(1)/dist/$(1)-$(2)"
endef

# -----------------------------------------------------------------------------
# Server + Target Targets (e.g., build-bash-darwin-arm64)
# -----------------------------------------------------------------------------
define make-server-target-rule
build-$(1)-$(2):
	$$(call build-server,$(1),$(2))
endef

$(foreach s,$(SERVERS),$(foreach t,$(TARGETS),$(eval $(call make-server-target-rule,$(s),$(t)))))

# -----------------------------------------------------------------------------
# Server Targets (e.g., build-bash) - builds all targets for a server
# -----------------------------------------------------------------------------
define make-server-rule
build-$(1): check-cross $(foreach t,$(TARGETS),build-$(1)-$(t))
	@echo "All targets built for $(1)"
endef

$(foreach s,$(SERVERS),$(eval $(call make-server-rule,$(s))))

# -----------------------------------------------------------------------------
# Build All
# -----------------------------------------------------------------------------
build-all: check-cross $(foreach s,$(SERVERS),build-$(s))
	@echo "All servers built for all targets"

# Convenience alias
build: build-all

# -----------------------------------------------------------------------------
# Clean
# -----------------------------------------------------------------------------
clean:
	@echo "Cleaning dist directories..."
	@$(foreach s,$(SERVERS),rm -rf packages/$(s)/dist;)
	@echo "Clean complete"

clean-all: clean
	@echo "Cleaning cargo target directories..."
	@$(foreach s,$(SERVERS),cd packages/$(s) && cargo clean;)
	@echo "Full clean complete"

# -----------------------------------------------------------------------------
# Help
# -----------------------------------------------------------------------------
help:
	@echo "Tool Servers Makefile"
	@echo "====================="
	@echo ""
	@echo "Targets:"
	@echo "  build-all              Build all servers for all platforms"
	@echo "  build                  Alias for build-all"
	@echo "  build-<server>         Build a single server for all platforms"
	@echo "  build-<server>-<target> Build a single server for a specific platform"
	@echo "  clean                  Remove dist directories"
	@echo "  clean-all              Remove dist and cargo target directories"
	@echo "  help                   Show this help message"
	@echo ""
	@echo "Servers: $(SERVERS)"
	@echo ""
	@echo "Platforms:"
	@echo "  darwin-arm64           macOS ARM64 (native)"
	@echo "  linux-arm64            Linux ARM64 (cross-compiled)"
	@echo "  linux-x86_64           Linux x86_64 (cross-compiled)"
	@echo ""
	@echo "Examples:"
	@echo "  make build-bash                    # Build bash for all platforms"
	@echo "  make build-bash-darwin-arm64       # Build bash for macOS ARM64 only"
	@echo "  make build-filesystem-linux-arm64  # Build filesystem for Linux ARM64"
	@echo "  make build-all                     # Build everything"
	@echo ""
	@echo "Prerequisites:"
	@echo "  - Rust toolchain"
	@echo "  - cross (cargo install cross)"
	@echo "  - Docker (for cross-compilation)"
