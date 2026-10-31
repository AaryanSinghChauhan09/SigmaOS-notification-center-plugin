# SigmaOS Notification Center Plugin

Python CLI and state model for bounded notification history, do-not-disturb
policy, and notification closing. It overlaps with the notification model in
`SigmaOS-notification`; settle their integration boundary before treating both
as independent daemons.

## Current integration limits

This repository does not export a D-Bus service. It sends visible alerts via
the external `notify-send` command. The default command receives app, urgency,
and summary; it does not receive the body. Each CLI invocation reconstructs
active state from a file that does not include active IDs, so closing a
notification from an earlier invocation is not supported.

## Development

`make test` exercises the state model without contacting the session bus. The
project is inspired by
[Omarchy's notification-center plugin](https://github.com/omacom/omarchy-notification-center-plugin)
but does not claim D-Bus service compatibility.
