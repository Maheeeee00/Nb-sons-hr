"""Read punches from the ZKTeco ZKTime / Attendance Management database (att2000.mdb).
Works without Microsoft Access installed (uses the bundled access_parser reader)."""
import os
import shutil
import string
import tempfile

from zk_vendor.access_parser import AccessParser

NAMES = ("att2000.mdb",)
COMMON = [r"Program Files (x86)\ZKTime5.0", r"Program Files\ZKTime5.0", r"ZKTime5.0", r"Program Files (x86)\Att",
          r"Program Files\Att", r"Att", r"Program Files (x86)\ZKTeco\Att", r"Program Files (x86)\Attendance Management",
          r"Program Files (x86)\ZK Software\Att", r"Program Files (x86)\ZKTeco\ZKTime5.0"]


def find_mdb(extra_dirs=()):
    """Return paths of ZKTime databases found on this PC (common folders first, then a short search)."""
    found = []
    roots = [f"{d}:\\" for d in string.ascii_uppercase[2:8] if os.path.exists(f"{d}:\\")] if os.name == "nt" else []
    for d in list(extra_dirs) + [os.path.join(r, c) for r in roots for c in COMMON]:
        for n in NAMES:
            p = os.path.join(d, n)
            if os.path.isfile(p) and p not in found:
                found.append(p)
    if not found:                                     # short search inside Program Files and drive roots
        for r in roots:
            for base in (os.path.join(r, "Program Files (x86)"), os.path.join(r, "Program Files"), r):
                try:
                    for top in os.listdir(base):
                        sub = os.path.join(base, top)
                        if os.path.isdir(sub) and any(k in top.lower() for k in ("zk", "att", "time")):
                            for dp, _dn, files in os.walk(sub):
                                if dp.count(os.sep) - sub.count(os.sep) > 2:
                                    continue
                                for f in files:
                                    if f.lower() in NAMES:
                                        found.append(os.path.join(dp, f))
                except OSError:
                    pass
    return found


def read_mdb(path):
    """Return (people, punches): people = {badge: name}; punches = [(badge, 'YYYY-MM-DD HH:MM:SS'), ...]."""
    tmp = os.path.join(tempfile.gettempdir(), "nbsons-zkt-copy.mdb")
    shutil.copyfile(path, tmp)                        # read a copy so ZKT keeps working undisturbed
    try:
        db = AccessParser(tmp)
        u = db.parse_table("USERINFO")
        c = db.parse_table("CHECKINOUT")
    finally:
        try:
            os.remove(tmp)
        except OSError:
            pass
    uid_badge = {}
    people = {}
    for uid, badge, name in zip(u.get("USERID", []), u.get("Badgenumber", []), u.get("Name", [])):
        badge = str(badge or "").strip()
        if badge:
            uid_badge[uid] = badge
            people[badge] = str(name or "").strip()
    punches = []
    for uid, t in zip(c.get("USERID", []), c.get("CHECKTIME", [])):
        if uid in uid_badge and t:
            punches.append((uid_badge[uid], str(t)[:19]))
    return people, punches
