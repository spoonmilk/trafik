#!/bin/bash

set -e

if ! sysctl net.ipv4.tcp_available_congestion_control | grep -q "ebpf_ccp_gen"; then
    echo "ebpf_ccp_gen not registered - nothing to clean up"
    exit 0
fi

echo "Attempting to cleanup"

DAEMON_PID=$(pgrep -f "ebpf-ccp-generic" || true)
if [ -n "$DAEMON_PID" ]; then
    echo "Stopping running PID: $DAEMON_PID)"
    kill -TERM "$DAEMON_PID" 2>/dev/null || true
    sudo bpftool struct_ops unregister name ebpf_ccp_gen

    for i in {1..10}; do
        if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
            echo "Shutting down daemon"
            break
        fi
        sleep 0.5
    done

    if kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo "Forcing daemon shutdown"
        kill -9 "$DAEMON_PID" 2>/dev/null || true
    fi
fi

sleep 10

if sysctl net.ipv4.tcp_available_congestion_control | grep -q "ebpf_ccp_gen"; then
    echo "ebpf_ccp_gen is still registered after daemon shutdown"
    echo "This can happen if the struct_ops link was not properly detached"
    echo ""
    echo "To fix this, you may need to:"
    echo "  1. Unload any manually pinned BPF objects: rm -f /sys/fs/bpf/ebpf_ccp*"
    echo "  2. Check for orphaned BPF programs: bpftool prog show"
    echo "  3. Reboot VM if the issue persists"
    echo ""
    echo "Current TCP CCA status:"
    sysctl net.ipv4.tcp_available_congestion_control
    exit 1
else
    echo "Cleanup successful - ebpf_cubic has been unregistered"
    echo "Current available CCAs:"
    sysctl net.ipv4.tcp_available_congestion_control
fi
