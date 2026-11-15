# Updates

Oxide keeps itself up to date from its git repository.

* A few minutes after you log in, and then every six hours, `oxide-ctl update auto` asks the repository whether
  there is a newer version (one small request). If there is, it fetches it into `~/.local/share/oxide/src`, builds it
  at the lowest CPU and disk priority and installs it like `contrib/install.sh` does. The build takes a few minutes
  and needs Rust (`cargo`) and `git`, the same as installing from source.
* The running session is not touched. The next time you log in, a card shows **Oxide was updated** with what
  changed; **Got it** hides it for good.
* *Settings → Config → Updates* shows the installed version and when it last looked, has **Check now** /
  **Update now**, and the **Update automatically** switch (`[update] auto`).
* A build from a copy with local changes, or from a commit that is not in the repository's history, is never
  replaced: it is marked as such in the settings and left alone.

From a terminal:

```sh
oxide-ctl update            # fetch, build and install now
oxide-ctl update check      # only look
oxide-ctl update status     # installed and newest version, last check, last error (JSON)
```

The log of the last build is `~/.local/state/oxide/update.log`; build files are kept in `~/.cache/oxide/build`
(delete it any time to free space).

The phone app updates itself from the GitHub releases: *Settings → Updates → Update* in the app.
