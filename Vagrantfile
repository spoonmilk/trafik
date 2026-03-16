# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure('2') do |config|
  # See docs:
  # https://docs.vagrantup.com.

  ### Box and Provider Config

  # I use VirtualBox but this
  # looks pretty similar on any of them
  config.vm.box = 'bento/ubuntu-24.04'
  config.vm.hostname = 'trafik-dev'
  config.vm.provider 'virtualbox' do |vb|
    vb.gui = false
    vb.memory = '4096'
    vb.cpus = 4
    vb.name = 'trafik-dev'
    vb.customize ['modifyvm', :id, '--nested-hw-virt', 'on']
  end
  config.vm.synced_folder '.', '/home/trafik', create: true

  ### Network Configurations

  # Use ifconfig to see ip
  config.vm.network 'private_network', type: 'dhcp'
  config.vm.network 'forwarded_port', guest: 80, host: 8080

  ### Utility Config Settings

  # Just some nice things - lets you use ssh keys and git inside vm
  # in case you develop in-vm
  config.ssh.forward_agent = true
  config.ssh.keep_alive = true

  # Support git operations inside the VM
  if File.exist?(File.expand_path('~/.gitconfig'))
    config.vm.provision 'file', source: '~/.gitconfig', destination: '~/.gitconfig'
  end

  # Make SSH public keys available inside the VM
  Dir.chdir(File.expand_path('~/.ssh')) do
    Dir.glob('*.pub').each do |pubkey|
      config.vm.provision 'file', source: File.join('~/.ssh', pubkey), destination: '/tmp/'
    end
  end

  config.vm.provision 'shell', path: './scripts/provision.sh',
                               env: { 'HOST_TIMEZONE' => Time.now.zone },
                               privileged: false,
                               binary: true # Don't convert to CRLF on Windows
end
