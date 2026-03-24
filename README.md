# TRAFIK!
 
> [!NOTE]
> This is an open research repository. It is very much in progress and much of the documentation
> below is currently deprecated! Expect things in this repo to be broken, half-baked, and badly
> documented for a while.

TRAFIK! is a project focused on making the implementation of congestion control algorithms (CCAs)
easier by having the user/developer only implement algorithm logic, handing off actual kernel code
to an eBPF framework the developer does not have to touch. It's heavily in-progress, and we're
currently exploring building a frontend and compiler for CCAs written in Rust.

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

Algorithms are (presently) written by implementing the GenericRunner trait for
their respective algorithm. In the future, this will be done entirely through
a library/DSL frontend.

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
sudo pkill -f ebpf-ccp-cubic
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

> This repo was created and written by Alex Khosrowshahi (Brown '27) for research
under Professor Akshay Narayan as part of the CCP project.
A large part of the code is taken from the `ccp-project/generic-cong-avoid` repository.

Notes on the usage of AI in this repository:
> I do not particularly enjoy LLMs. I consciously avoid their usage in many cases.
That said, when I find some use for them, such as acting as smart documentation for
somewhat sparsely documented eBPF features/libraries, I do make use of them.
In addition, I have used them to generate some amount of code in this repository, mostly configuration.
What I can guarantee is that there has been none/is no "vibe coding" present in this work
(that is: generated code not manually verified and understood by a human developer).
