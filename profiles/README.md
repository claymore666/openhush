# Security Profiles for OpenHush

This directory contains security sandbox profiles for Linux. These profiles restrict OpenHush to only the resources it needs, reducing the impact of potential security vulnerabilities.

## Why Sandbox?

OpenHush processes audio input and uses FFI bindings to native libraries (whisper.cpp). Sandboxing provides defense-in-depth:

- **Limits file access** to config, models, and logs only
- **Restricts network** to localhost (Ollama API)
- **Blocks access** to sensitive data (SSH keys, browser profiles, keyrings)
- **Contains potential exploits** within the sandbox

## Quick Start

### Firejail (Easiest)

```bash
# Install Firejail
sudo apt install firejail  # Debian/Ubuntu
sudo dnf install firejail  # Fedora
sudo pacman -S firejail    # Arch

# Install profile
sudo cp profiles/firejail/openhush.profile /etc/firejail/

# Run sandboxed
firejail openhush start
```

### AppArmor (Ubuntu/Debian/SUSE)

```bash
# Install profile
sudo cp profiles/apparmor/openhush /etc/apparmor.d/usr.bin.openhush

# Load profile
sudo apparmor_parser -r /etc/apparmor.d/usr.bin.openhush

# Verify
sudo aa-status | grep openhush
```

### SELinux (Fedora/RHEL)

```bash
# Build policy module
cd profiles/selinux
checkmodule -M -m -o openhush.mod openhush.te
semodule_package -o openhush.pp -m openhush.mod

# Install module
sudo semodule -i openhush.pp

# Label the binary
sudo semanage fcontext -a -t openhush_exec_t '/usr/bin/openhush'
sudo restorecon -v /usr/bin/openhush

# Verify
ps -eZ | grep openhush
```

## Profile Comparison

| Feature | AppArmor | SELinux | Firejail |
|---------|----------|---------|----------|
| Ease of use | Medium | Hard | Easy |
| Granularity | Path-based | Label-based | Namespace-based |
| Default on | Ubuntu, SUSE | Fedora, RHEL | None |
| Requires root | Install only | Install only | No (usually) |
| Container support | Yes | Yes | Limited |

## Permissions Granted

All profiles allow:

| Resource | Permission | Reason |
|----------|------------|--------|
| `~/.config/openhush/` | Read/Write | Configuration |
| `~/.local/share/openhush/` | Read/Write | Models, logs, cache |
| `/dev/snd/*` | Read/Write | Microphone capture |
| `/dev/nvidia*` | Read/Write | CUDA GPU access |
| `/dev/dri/*` | Read/Write | GPU access (AMD/Intel) |
| D-Bus Notifications | Send | Desktop notifications |
| D-Bus StatusNotifier | Send | System tray |
| localhost:* | Connect | Ollama API |

## Permissions Denied

All profiles block:

| Resource | Reason |
|----------|--------|
| `~/.ssh/` | SSH keys |
| `~/.gnupg/` | GPG keys |
| `~/.mozilla/`, `~/.config/chromium/` | Browser data |
| `~/.local/share/keyrings/` | Password storage |
| `/etc/shadow` | System passwords |
| Other home directories | Privacy |

## Troubleshooting

### AppArmor

```bash
# Check for denials
sudo dmesg | grep -i apparmor | grep DENIED

# Temporarily disable for testing
sudo aa-complain /etc/apparmor.d/usr.bin.openhush

# Re-enable
sudo aa-enforce /etc/apparmor.d/usr.bin.openhush

# Remove profile completely
sudo apparmor_parser -R /etc/apparmor.d/usr.bin.openhush
sudo rm /etc/apparmor.d/usr.bin.openhush
```

### SELinux

```bash
# Check for denials
sudo ausearch -m AVC -ts recent | grep openhush

# Generate allow rules from denials
sudo ausearch -m AVC -ts recent | audit2allow -M openhush_local
sudo semodule -i openhush_local.pp

# Set to permissive (log only, don't block)
sudo semanage permissive -a openhush_t

# Remove permissive
sudo semanage permissive -d openhush_t

# Remove module
sudo semodule -r openhush
```

### Firejail

```bash
# Run with debug output
firejail --debug openhush start

# Check what's blocked
firejail --trace openhush start

# Run without profile (unsandboxed)
firejail --noprofile openhush start

# List active sandboxes
firejail --list
```

## Runtime Detection

OpenHush detects if it's running in a sandbox and logs this at startup:

```
INFO Running in AppArmor sandbox - security profile active
```

You can check programmatically:

```rust
use openhush::platform::sandbox::{detect_sandbox, SandboxType};

let sandbox = detect_sandbox();
if sandbox.is_sandboxed() {
    println!("Running in {} sandbox", sandbox.name());
}
```

## Custom Profiles

To customize a profile for your setup:

1. Copy the profile to a local location
2. Modify permissions as needed
3. Test with logging enabled
4. Install the modified profile

### Adding Permissions

**AppArmor:**
```
# Add to profile
owner /path/to/extra/resource rw,
```

**SELinux:**
```
# Generate rule from denial
audit2allow -M mypolicy < /var/log/audit/audit.log
```

**Firejail:**
```
# Add to profile
whitelist /path/to/extra/resource
noblacklist /path/to/extra/resource
```

## Security Notes

1. **Profiles are not perfect** - They reduce attack surface but don't guarantee security
2. **Keep profiles updated** - New features may need new permissions
3. **Test after updates** - Profile changes can break functionality
4. **Report issues** - If you find a needed permission, open an issue

## References

- [AppArmor Wiki](https://gitlab.com/apparmor/apparmor/-/wikis/home)
- [SELinux Project](https://selinuxproject.org/)
- [Firejail Documentation](https://firejail.wordpress.com/documentation-2/)
- [Landlock (future)](https://landlock.io/)
