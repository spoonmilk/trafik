.PHONY: all ebpf rust clean cleanup help
.PHONY: list-algorithms check-algorithms run
.PHONY: test test-register test-quick test-full test-all-algorithms
.PHONY: test-cubic test-reno test-generic-cubic test-generic-reno
.PHONY: test-quick-cubic test-quick-reno test-quick-generic-cubic test-quick-generic-reno
.PHONY: test-full-cubic test-full-reno test-full-generic-cubic test-full-generic-reno

BINARY := ./target/release/ebpf-ccp-generic

## ── Help ──────────────────────────────────────────────────────────────────────

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Build"
	@echo "  all                   Build eBPF programs and Rust daemon (full build)"
	@echo "  ebpf                  Build only the eBPF kernel programs"
	@echo "  rust                  Build only the Rust userspace daemon"
	@echo "  clean                 Remove all build artifacts"
	@echo ""
	@echo "Run"
	@echo "  run                   Build and run the daemon (default algorithm: cubic)"
	@echo "  list-algorithms       List available congestion control algorithms"
	@echo ""
	@echo "Test  (require sudo, run inside VM)"
	@echo "  test-register         Verify eBPF struct_ops attachment works"
	@echo "  test / test-quick     3-second iperf test (default algorithm)"
	@echo "  test-full             10-second iperf test with packet drops"
	@echo "  test-all-algorithms   Cycle through all algorithms with quick tests"
	@echo ""
	@echo "  Algorithm-specific quick tests:"
	@echo "    test-quick-cubic  test-quick-reno  test-quick-generic-cubic  test-quick-generic-reno"
	@echo ""
	@echo "  Algorithm-specific full tests:"
	@echo "    test-full-cubic   test-full-reno   test-full-generic-cubic   test-full-generic-reno"
	@echo ""
	@echo "Utilities"
	@echo "  cleanup               Kill daemon and detach eBPF programs (run if 'File exists' errors appear)"
	@echo ""
	@echo "Environment variables (for test targets):"
	@echo "  ALGORITHM    Algorithm name (cubic, reno, generic-cubic, generic-reno) [default: cubic]"
	@echo "  TEST_DURATION, LATENCY, BANDWIDTH, LOSS  — passed through to scripts/test.sh"

## ── Build ─────────────────────────────────────────────────────────────────────

all: ebpf rust

ebpf:
	@echo "Building eBPF programs..."
	$(MAKE) -C ebpf

rust: ebpf
	@echo "Building Rust daemon..."
	cargo build --release

clean:
	$(MAKE) -C ebpf clean
	cargo clean

## ── Run ───────────────────────────────────────────────────────────────────────

run: all
	sudo $(BINARY)

list-algorithms: rust
	$(BINARY) --list-algorithms

check-algorithms: rust
	$(BINARY) --list-algorithms

## ── Test ──────────────────────────────────────────────────────────────────────

cleanup:
	sudo ./scripts/cleanup-named.sh
	sudo ./scripts/cleanup_generic.sh

test-register: cleanup all
	sudo ALGORITHM=generic-cubic ./scripts/test.sh basic

# Quick tests (3 seconds)
test-quick: cleanup all
	sudo ./scripts/test.sh quick

test-quick-cubic: cleanup all
	sudo ALGORITHM=cubic ./scripts/test.sh quick

test-quick-reno: cleanup all
	sudo ALGORITHM=reno ./scripts/test.sh quick

test-quick-generic-cubic: cleanup all
	sudo ALGORITHM=generic-cubic ./scripts/test.sh quick

test-quick-generic-reno: cleanup all
	sudo ALGORITHM=generic-reno ./scripts/test.sh quick

# Full 10-second tests
test-full: cleanup all
	sudo ./scripts/test.sh full

test-full-cubic: cleanup all
	sudo ALGORITHM=cubic ./scripts/test.sh full

test-full-reno: cleanup all
	sudo ALGORITHM=reno ./scripts/test.sh full

test-full-generic-cubic: cleanup all
	sudo ALGORITHM=generic-cubic ./scripts/test.sh full

test-full-generic-reno: cleanup all
	sudo ALGORITHM=generic-reno ./scripts/test.sh full

test-cubic: test-quick-cubic
test-reno: test-quick-reno
test-generic-cubic: test-quick-generic-cubic
test-generic-reno: test-quick-generic-reno

test-all-algorithms: cleanup all
	@sudo ALGORITHM=cubic ./scripts/test.sh quick
	@sudo ./scripts/cleanup-named.sh
	@sleep 2
	@sudo ALGORITHM=reno ./scripts/test.sh quick
	@sudo ./scripts/cleanup-named.sh
	@sleep 2
	@sudo ALGORITHM=generic-cubic ./scripts/test.sh quick
	@sudo ./scripts/cleanup_generic.sh
	@sleep 2
	@sudo ALGORITHM=generic-reno ./scripts/test.sh quick
	@sudo ./scripts/cleanup_generic.sh

test: test-quick

## ──────────────────────────────────────────────────────────────────────────────
