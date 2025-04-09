# Utility to sync Logitech devices profiles

## Rationale

*Logi Options+* software for Logitech devices maintains per-device settings.
Maybe it is useful for someone, but two important scenarios are completely broken:

* Same device at home/work

  I really like *Logitech MX Master 3s* mouse and have two of them: one at home and another at work.
  And I want them to behave exactly same way independent of the location. There are lots of per-application
  settings and it's pain to sync them manually.

* Lost/broken device

  Say, I lost or break mouse and bought another one. How to transfer settings to it?

Seems I'm not the only one (e.g. [here](https://www.reddit.com/r/logitech/comments/1db2924/migrate_setting_from_one_mouse_to_another/),
[here](https://apple.stackexchange.com/questions/427650/how-can-i-copy-logi-options-application-settings-from-one-device-to-another-devi)
and [here](https://logitech.uservoice.com/forums/925117-logi-options/suggestions/45132478-export-settings-for-import-with-new-device))
looking for a way to sync settings between devices, but this feature isn't implemented yet.

## Disclaimer

This utility is written for my needs primarily. It works for me, but it's rather rudimentary.
There is no detailed help, logging, safety checks.
It exploits undocumented details of *Logi Options+* implementation and may break your configuration
(however, it backs up settings before modification).
It is intended to work on MacOS. It mustn't be hard to make it work on Windows, but I need someone
to help me test it.
It overrides settings of target device without warning!

## Usage

Currenly there are no automatic builds, so you need `cargo` tool to build.

1. List devices

  ```bash
  ❯ cargo run -- ~/Library/Application\ Support/LogiOptionsPlus/settings.db list-devices
  m337-1b016: M336 / M337 / M535
  mx-master-3-6b023: MX Master 3
  mx-master-3s-2b034: MX Master 3S
  ```

2. Transfer settings
  ```bash
  ❯ cargo run -- ~/Library/Application\ Support/LogiOptionsPlus/settings.db transfer-assignments mx-master-3s-2b034 mx-master-3-6b023s
  ```

It you are lucky, settings are synced now.
