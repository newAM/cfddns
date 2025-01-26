# cfddns

Cloudflare dynamic DNS client for centralized IPv6 updates on a home network.

This was created to solve a specific problem I had with IPv6 dynamic DNS on my home network.

All DDNS clients I found will only update records for the host they are running on.

- With IPv4 this is fine because all devices share a public IPv4 with NAT.
- With IPv6 this is problematic because many devices are incapable of running a DDNS client.

`cfddns` centralizes all IP updates for a home network on a single host.

- A records are set to the same public IPv4.
- AAAA records are created using the hosts dynamic IPv6 prefix, and the clients static IPv6 suffix.

If you have the same problem `cfddns` may be for you.
For all other use-cases I recommend [ddclient] because it's much more flexible.

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

## NixOS configuration

Add to your flake inputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    cfddns.url = "github:newAM/cfddns";
    cfddns.inputs.nixpkgs.follows = "nixpkgs";
    cfddns.inputs.advisory-db.follows = "";
    cfddns.inputs.treefmt.follows = "";
  };

  # ...
}
```

Reference `nixos/module.nix` for a complete list of options,
below is an example of my configuration.

```nix
{
  cfddns,
  config,
  ...
}: {
  # import the module, this adds the "services.cfddns" options
  imports = [cfddns.nixosModules.default];

  # add the overlay, this puts "cfddns" into "pkgs"
  nixpkgs.overlays = [cfddns.overlays.default];

  # use nix-sops to manage secrets declaratively
  # https://github.com/Mic92/sops-nix
  sops.secrets.cfddns.mode = "0400";

  # reference module for descriptions of configuration
  services.cfddns = {
    enable = true;
    environmentFiles = [config.sops.secrets.cfddns.path];
    settings = {
      a_interface = "bond-wan";
      aaaa_interface = "br-lan";
      zones = [
        {
          name = "example.com";
          records = [
            {
              name = "service.example.com";
              eui64 = "::aaaa:aaaa:aaaa:aaaa";
            }
          ];
        }
      ];
    };
  };
}
```

[ddclient]: https://github.com/ddclient/ddclient
