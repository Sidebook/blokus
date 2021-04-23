# Prerequisite

Rust

# How to play locally

```
$ cargo run play
```

For 2-player game,
```
$ carfo run -- -m duo play
```

# How to host a game
```
$ cargo run host
```

# How to join a existing game
```
$ cargo run join [url] -p [player-id]
```

Join with a specific player name
```
$ cargo run -- -name [your name] join [url] -p [player-id]
```