# NB Sons HR – Desktop apps (Tauri)

* **NB Sons HR Server** – install on the ONE main PC. Runs the HR system, keeps all data in
  `Documents\NB Sons HR Data`, starts with Windows, lives in the tray, opens firewall port 5000.
* **NB Sons HR** (client) – install on every other PC. Finds the server automatically or asks for its
  address once, remembers it, shows "can't reach server" and reconnects by itself. Stores no data.

## Build (free, no tools needed on your PC)
1. Create a free GitHub account, make a new **private** repository, upload all files of this folder
   (keep the folder structure, including `.github`).
2. GitHub builds automatically (Actions tab, ~15–20 min).
3. Download both Setup .exe files from **Releases**.

## Move your existing data
Before first start of the Server app, copy your old `hr.db` into `Documents\NB Sons HR Data`.
Close the old "Start Server" window first (both use port 5000).

Windows may say "unrecognized app" (installers aren't code-signed): click **More info → Run anyway**.
