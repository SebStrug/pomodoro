# Pomodoro

Pomodoro on terminal

1. Start with `cargo run`
1. Will run for 25 minutes
1. Beeps
1. Beeping continues every 10 seconds until ack'd with `cargo run ack`
1. Break begins for 10 minutes
1. Break ends, will wait until ack'd with `cargo run ack`.
1. Repeats until `cargo run no more`

Your pomodoros are logged to `~/.pomodoro-stats`.

Caveat emptor, only tried on Mac.