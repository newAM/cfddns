# cfddns

Cloudflare Dynamic DNS client.

This was created to solve a specific problem I had with IPv6 dynamic DNS on my home network.

All DDNS clients I found will only update records for the host they are running on.

- With IPv4 this is fine because all devices share a public IPv4 with NAT.
- With IPv6 this is problematic because many devices are incapable of running a DDNS client.

`cfddns` centralizes all IP updates for a home network on a single host.

- A records are set to the same public IPv4.
- AAAA records are created using the hosts dynamic IPv6 prefix, and the clients static IPv6 suffix.

## Features

- NixOS module provided
- Supports IPv4 and/or IPv6
- Supports obtaining IP address from an interface, or from an HTTP service such as <https://icanhazip.com>

### Limitations

- Linux only
- Systemd unit is provided for NixOS only
- Built for a home network with a single IPv4 and/or a single IPv6 prefix delegation
- Assumes IPv6 addresses are allocated with SLAAC or similar mechanism with known IPv6 suffixes
  - Doesn't support IPv6 privacy extensions
- Only supports Cloudflare
