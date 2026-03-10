# TRAFIK!

## Dependencies

The provided Vagrantfile should provide everything
except for cargo and libbpf-sys.
Make sure vmlinux is supported on your kernel version.

## Architecture

This project originally stemmed from efforts to implement the CCP project
in eBPF by re-implementing libccp in eBPF, though this proved infeasible with
verifier restrictions. 

Previous attempts have used a userspace libccp hooked to an eBPF program
which implemented struct_ops updates, but required a full userspace libccp
running process throughout. This implementation removes that layer of indirection,
and also implements a generic datapath program that can support a variety of
algorithms.

### Components

1. **eBPF Datapath** (`ebpf/datapath.bpf.c`): Kernel-space BPF program that:
   - Registers as a TCP congestion control algorithm via `struct_ops`
   - Hooks into TCP stack callbacks
   - Sends measurements and flow events to userspace via ring buffers
   - Receives cwnd updates from userspace via BPF hash map

2. **Rust Userspace Daemon** (`src/main.rs`, `src/bpf.rs`):
   - Loads and attaches the eBPF program using libbpf-rs
   - Polls ring buffers for measurements and flow events
   - Runs CUBIC algorithm logic per-flow
   - Sends cwnd updates back to kernel via BPF maps

3. **Algorithm Implementations** (`src/algorithms`):
   - Implement a generic algorithm/runner trait that act as userspace policy.

## Running

> [!NOTE]
> PLEASE DO NOT RUN THIS OUTSIDE OF A VIRTUAL MACHINE! IT CAN AND MIGHT BREAK
> YOUR KERNEL IN WAYS I REALLY DON'T WANT TO BE HELD RESPONSIBLE FOR, OKEY?

Once you've got your VM up and running (I built on Ubuntu 24.04) and have
all your dependencies satisfied, you can run `make all` to build both the
eBPF and userspace Rust component.

There are three different configurations for testing functionality.

```bash
make test-register # Run registration test, succeeds if ebpf-cubic hooks into struct_ops
make test/test-quick # 3-second iperf test
make test-full # 10 second iperf test w/ packet drops
```

> [!NOTE]
> YOU MUST RUN WITH SUDO. IF THE MAKEFILE DOESN'T WORK FOR YOU, RUN THE SCRIPT
> MANUALLY UNDER SUDO

## Troubleshooting

### "File exists" error when running tests

```bash
make cleanup
```

Test targets automatically cleanup ebpf daemon on run

### Manual cleanup

If the automated cleanup fails, you can manually stop the daemon:

```bash
# Find and kill the daemon process
sudo pkill -f ebpf-ccp-cubic

# Verify ebpf_cubic is unregistered
sysctl net.ipv4.tcp_available_congestion_control
```

### Can't build because of cargo stuff

Solution:

```bash
rm -rf ~/.cargo/git
rm -rf ~/.cargo/registry
```

### Can't halt VM

Solution:

Still trying to fix this one. I just power the VM off, pray, then restart

This repo was created and written by Alex Khosrowshahi (Brown '27) for research
under Professor Akshay Narayan as part of the CCP project.
A large part of the code is taken from the `ccp-project/generic-cong-avoid` repository.
