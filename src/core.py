"""Notification daemon & centre for SigmaOS-notification and
SigmaOS-notification-center-plugin (inspired by omapager and the
org.freedesktop.Notifications spec).

Principles: monotonic ids, bounded history (no unbounded growth), do-not-disturb
policy with critical-pierces-DND (least astonishment), fail-fast on unknown
ids, pure state transitions with a thin D-Bus apply layer.
"""
import argparse, json, os, sys, tempfile

ENGINE = {
    "name": "sigma-notify",
    "summary": "Notification daemon: send, history, do-not-disturb, close",
    "language": "python",
    "verbs": ["send", "history", "dnd", "close"],
    "principles": ["Least Astonishment", "Defensive Programming", "Fail-Fast",
                   "KISS", "Separation of Concerns", "Deterministic Testing"],
}

cli_doc = """\
$ sigma-notify send --summary "Updates ready" --body "12 packages" --app sigma-update
#1 [normal] sigma-update: Updates ready - 12 packages
$ sigma-notify dnd on
$ sigma-notify history --limit 5
$ sigma-notify close --id 1
"""

DEFAULT_STORE = os.path.expanduser("~/.config/sigma-notify/centre.json")
URGENCIES = ("low", "normal", "critical")


class NotifyError(ValueError):
    pass


class NotificationCenter:
    def __init__(self, maxlen=50):
        self.next_id = 1
        self.dnd = False
        self.maxlen = int(maxlen)
        self.history = []   # oldest first, bounded
        self.active = {}    # id -> notification (awaiting close)


def send(center, summary, body="", app="sigma", urgency="normal"):
    """Create a notification. DND suppresses display but always logs."""
    if urgency not in URGENCIES:
        raise NotifyError(f"bad urgency: {urgency} (have: {', '.join(URGENCIES)})")
    if not str(summary).strip():
        raise NotifyError("empty summary")
    n = {"id": center.next_id, "summary": str(summary), "body": str(body),
         "app": str(app), "urgency": urgency,
         "shown": (not center.dnd) or urgency == "critical"}
    center.next_id += 1
    center.history.append(n)
    if len(center.history) > center.maxlen:
        del center.history[: len(center.history) - center.maxlen]
    if n["shown"]:
        center.active[n["id"]] = n
    return n


def history(center, limit=10):
    """Newest-first history slice. Pure."""
    if limit < 1:
        raise NotifyError("limit must be >= 1")
    return list(reversed(center.history))[:limit]


def set_dnd(center, on):
    center.dnd = bool(on)
    return center.dnd


def close(center, nid):
    """Close an active notification. Fail-fast on unknown ids."""
    if nid not in center.active:
        raise NotifyError(f"unknown active notification: {nid}")
    return center.active.pop(nid)


def render_notification(n):
    mark = "" if n["shown"] else " [suppressed: DND]"
    return (f"#{n['id']} [{n['urgency']}] {n['app']}: {n['summary']} - {n['body']}{mark}")


def load_center(path, maxlen=50):
    center = NotificationCenter(maxlen=maxlen)
    if os.path.exists(path):
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
        center.next_id = max(1, int(data.get("next_id", 1)))
        center.dnd = bool(data.get("dnd", False))
        center.history = list(data.get("history", []))[-maxlen:]
    return center


def save_center(path, center):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    data = {"next_id": center.next_id, "dnd": center.dnd, "history": center.history}
    tmp = path + ".sigma-tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        json.dump(data, fh, indent=2)
    os.replace(tmp, path)


def deliver(n, bus=None):
    """Execute layer: push a notification onto the real session bus.

    self_test NEVER calls this without an injected (recording) bus.
    """
    if bus is None:
        import subprocess
        bus = lambda rec: subprocess.run(
            ["notify-send", "-a", rec["app"], "-u", rec["urgency"], rec["summary"]],
            check=True)
    bus(n)
    return n["id"]


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog=ENGINE["name"], description=ENGINE["summary"])
    p.add_argument("--store", default=DEFAULT_STORE, help="centre store (JSON)")
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("send", help="send a notification")
    s.add_argument("--summary", required=True)
    s.add_argument("--body", default="")
    s.add_argument("--app", default="sigma")
    s.add_argument("--urgency", choices=URGENCIES, default="normal")
    h = sub.add_parser("history", help="show newest-first history")
    h.add_argument("--limit", type=int, default=10)
    d = sub.add_parser("dnd", help="get/set do-not-disturb")
    d.add_argument("state", nargs="?", choices=("on", "off", "status"), default="status")
    c = sub.add_parser("close", help="close an active notification")
    c.add_argument("--id", type=int, required=True)
    return p


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)
    try:
        center = load_center(args.store)
        if args.cmd == "send":
            n = send(center, args.summary, body=args.body, app=args.app,
                     urgency=args.urgency)
            if n["shown"]:
                deliver(n)
            print(render_notification(n))
            save_center(args.store, center)
        elif args.cmd == "history":
            for n in history(center, args.limit):
                print(render_notification(n))
        elif args.cmd == "dnd":
            if args.state == "on":
                set_dnd(center, True)
            elif args.state == "off":
                set_dnd(center, False)
            save_center(args.store, center)
            print(f"dnd = {'on' if center.dnd else 'off'}")
        elif args.cmd == "close":
            n = close(center, args.id)
            save_center(args.store, center)
            print(f"closed #{n['id']}")
        return 0
    except (NotifyError, OSError, ValueError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


def self_test() -> None:
    center = NotificationCenter(maxlen=3)
    # monotonic ids, shown by default
    n1 = send(center, "Hello", body="world", app="test")
    n2 = send(center, "Second")
    assert (n1["id"], n2["id"]) == (1, 2)
    assert n1["shown"] is True and set(center.active) == {1, 2}
    # DND suppresses normal but critical pierces (least astonishment)
    set_dnd(center, True)
    n3 = send(center, "Quiet", urgency="low")
    n4 = send(center, "ALARM", urgency="critical")
    assert n3["shown"] is False and 3 not in center.active
    assert n4["shown"] is True and 4 in center.active
    assert "[suppressed: DND]" in render_notification(n3)
    set_dnd(center, False)
    # bounded history, newest-first
    assert len(center.history) == 3            # maxlen=3 keeps last 3
    assert [n["id"] for n in history(center, 10)] == [4, 3, 2]
    assert [n["id"] for n in history(center, 2)] == [4, 3]
    try:
        history(center, 0)
    except NotifyError:
        pass
    else:
        raise AssertionError("bad history limit not caught")
    # closing: fail-fast on unknown ids
    assert close(center, 4)["urgency"] == "critical"
    try:
        close(center, 4)
    except NotifyError:
        pass
    else:
        raise AssertionError("double close not caught")
    # fail-fast input validation
    for bad in (lambda: send(center, "   "),
                lambda: send(center, "x", urgency="asap")):
        try:
            bad()
        except NotifyError:
            pass
        else:
            raise AssertionError("expected NotifyError")
    # persistence round-trip (tempdir only)
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "centre.json")
        save_center(path, center)
        again = load_center(path, maxlen=3)
        assert again.next_id == 5              # ids never reused
        assert [n["id"] for n in history(again, 10)] == [4, 3, 2]
        # execute layer with an injected (recording) bus
        ids = []
        assert deliver(n1, bus=lambda rec: ids.append(rec["id"])) == 1
        assert ids == [1]


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
