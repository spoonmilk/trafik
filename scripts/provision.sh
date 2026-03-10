#!/bin/bash

# Update packages
sudo DEBIAN_FRONTEND=noninteractive apt-get -qq update

# Install kernel headers
sudo DEBIAN_FRONTEND=noninteractive apt-get -qq -y install \
    linux-headers-$(uname -r) \
    linux-tools-$(uname -r) \
    linux-tools-generic \
    linux-tools-common

# Install (in my opinion) essential utilities
sudo DEBIAN_FRONTEND=noninteractive apt-get -qq -y install \
    build-essential rsync htop pv vim tmux ltrace strace \
    curl git zip unzip net-tools dnsutils gcc clang wget \
    python3 python3-pip python3-venv \


# Install eBPF dependencies
sudo DEBIAN_FRONTEND=noninteractive apt-get -qq -y install \
    clang \
    llvm \
    libbpf-dev \
    bpfcc-tools \
    python3-bpfcc \
    bpftool

# Network tools
sudo DEBIAN_FRONTEND=noninteractive apt-get -qq -y install \
    tcpdump \
    iperf3 \
    netcat \
    mininet \
    socat \
    jq \
    tshark \


# A lot of the following is taken from https://github.com/carlosefr/vagrant-templates

# Set timezone to match host
if timedatectl list-timezones | grep -qxF "${HOST_TIMEZONE:-"UTC"}"; then
    sudo timedatectl set-timezone "${HOST_TIMEZONE:-"UTC"}" || true
else
    sudo timedatectl set-timezone UTC || true
fi
echo "VM local timezone: $(timedatectl | awk '/[Tt]ime\s+zone:/ {print $3}')"
sudo systemctl -q enable systemd-timesyncd
sudo systemctl start systemd-timesyncd

# Minimize the number of running daemons (not needed in this headless VM)...
sudo systemctl -q stop iscsid.socket iscsid.service >/dev/null 2>&1 || true
sudo DEBIAN_FRONTEND=noninteractive apt-get -qq -y autoremove --purge \
    lxcfs snapd open-iscsi mdadm accountsservice acpid \
    multipath-tools modemmanager udisks2 fwupd upower

# Same, but this needs to be explicitly stopped before uninstalling...
if [[ -f /usr/lib/packagekit/packagekitd || -f /usr/libexec/packagekitd ]]; then
    sudo systemctl -q is-active packagekit && sudo systemctl stop packagekit
    sudo DEBIAN_FRONTEND=noninteractive apt-get -qq -y purge packagekit
fi

# Prevent locale forwarding issues
if sudo grep -q '^AcceptEnv\s.*LC_' /etc/ssh/sshd_config; then
    sudo sed -i 's/^\(AcceptEnv\s.*LC_\)/#\1/' /etc/ssh/sshd_config
fi
echo "PubkeyAcceptedKeyTypes +ssh-rsa" | sudo tee "/etc/ssh/sshd_config.d/vagrant_ssh_rsa.conf" >/dev/null

sudo systemctl restart ssh

# Set up SSH keys from host
echo "Setting up SSH keys..."
if ls -1 /tmp | grep -qE '^id_[^.]+\.pub$|.*\.pub$'; then
    pushd "${HOME}/.ssh" >/dev/null

    # Backup original authorized_keys
    if [[ ! -f .authorized_keys.vagrant ]]; then
        cp authorized_keys .authorized_keys.vagrant 2>/dev/null || touch .authorized_keys.vagrant
    fi

    # Add host SSH keys
    cat .authorized_keys.vagrant /tmp/*.pub > authorized_keys 2>/dev/null || true
    chmod 0600 authorized_keys
    rm -f /tmp/*.pub

    popd >/dev/null
    echo "SSH keys from host installed successfully"
fi

# Remove mail notification
if [[ -s "/var/spool/mail/${USER}" ]]; then
    echo -n > "/var/spool/mail/${USER}"
fi

clang --version
bpftool --version

# Install Rust via rustup
echo "Installing Rust toolchain..."
if ! command -v cargo &>/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
else
    echo "Rust already installed: $(cargo --version)"
fi
cargo --version

