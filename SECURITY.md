# Security

If you find a security or privacy problem in Memi, please email **hello@yourmemi.app** instead of opening a public issue. Include what you found, how to reproduce it, and which version and platform you used.

You'll get a reply within a week. Once a fix is released, we're happy to credit you in the release notes if you'd like.

Areas of particular interest:

- The sync encryption and key handling (`app/src/main/java/app/memi/sync/`, `desktop/src/sync/`)
- Pairing a computer by QR code
- Anything that sends data off the device without the user turning on sync
- Importing backups and photos
