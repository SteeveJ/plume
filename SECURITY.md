# Security policy

**English** | [Français](SECURITY.fr.md)

## Reporting a vulnerability

Please don't open a public issue for a security problem. Use GitHub's private reporting instead: on the repository page, open the **Security** tab and choose **Report a vulnerability**.

A good report says what you found, how to reproduce it, and what an attacker gains. The commit you tested (`git rev-parse HEAD`) and the relevant lines of your `plume.conf` help a lot. Leave out real API keys and anything you dictated that you'd rather keep private.

Plume is maintained by one person. Expect a first answer within about a week. Fixes take as long as they take, and anything that exposes keys, dictated text or the machine comes first. Once a fix is out, the report can be made public, and I'll credit you if you want that.

## Supported versions

Plume has no tagged releases yet, so only the latest commit on `main` gets fixes.

## What counts

In scope:
- Handling of API keys, the config file, the history and the scratchpad (permissions, leaks, logs).
- Anything that makes Plume send more data to an online service than the README says it does.
- Model downloads and the files Plume loads from them.
- The install step, the restart helper and the use of `uinput` and the `input` group.
- The build script in `vendor/whisper-rs-sys/`, which rewrites the whisper.cpp archives.

Out of scope:
- The trade-offs already listed under "Known weak spots" in the [README](README.md#security-and-privacy): the `input` group is wider than Plume, secrets sit in plain text with mode 600, and the process isn't sandboxed. If you can make one of them worse, that's in scope.
- Problems that need root or physical access to the machine.
- Bugs in whisper.cpp, llama.cpp or the models themselves. Report them upstream, and tell me if the way Plume uses them makes things worse.
- Whisper inventing text on noise. That's a quality issue, tracked in the tests, unless it leaks data.

## Testing on your own

Test on your own machine and your own accounts. Don't point Plume at someone else's data or services.
